//! The marine HF receiver (`marine`): NAVTEX, MF/HF DSC, VHF channel 70 DSC and HF weather fax on one tuned channel, with the
//! service selection, the FSK finder, parallel DSC channels, the DSC scanner and the typed results.

use crate::dsc::call::{self as dsccall, DscCall};
use crate::dsc::channels::{self, Scanner};
use crate::dsc::decoder::DscDecoder;
use crate::dsp::FixedRateFrontEnd;
use crate::fax::{FaxDecoder, FaxSettings, FaxStatus, UsbAudio, MAX_LINES};
use crate::fsk::{AfskSlicer, FskFinder, FskSeen, MfSlicer, CHANNEL_RATE};
use crate::navtex::{Framer, MessageList, NavtexMessage, SitorDecoder};
use crate::report::{Clock, Hearing};
use sdrkit_algo_modem::follow::{DiscFollowConfig, DiscriminatorFollow};
use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet, Commands};
use sdrkit_core::interface::{EventKind, Fields, Hertz, Seconds, Table, Time};
use sdrkit_core::media::Picture;
use sdrkit_core::publish::keys::{Family, Urgency};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::results::copy_if_changed;
use sdrkit_core::{
    Choice, Complex32, ConfigError, DeclareSetting, Event, HostRequest, Record, Results, Schema,
    Setting, SettingError, SettingId, SettingSet, StackConfig, StateBlob, StateError, Status,
    Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, Applied, Catalog, Component, ComponentInfo, Format, GraphBuilder,
    GraphError, Io, Item, Kind, Port, RateFrom, Registry, RunContext, StackGraph, Timed,
};
use sdrkit_util_ids::names::{IdKind, IdNames};
use sdrkit_util_objects::commands::{self as objcmd, Tables};
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, LogRing, Seen, Shift, Stamp};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The lowest input rate.
pub const MIN_RATE_HZ: f64 = 12e3;
/// The report interval.
pub const REPORT_S: f64 = 0.25;
/// The tune offset: the radio is tuned 20 kHz above the dial.
pub const TUNE_OFFSET_HZ: f64 = 20e3;
/// The most DSC calls listed.
pub const DSC_CAP: usize = 100;

sdrkit_core::choice_enum! {
    /// Which decoders of the marine HF receiver run.
    pub enum Service {
        /// Chosen from the dial.
        Auto = ("auto", "Automatic"),
        /// NAVTEX.
        Navtex = ("navtex", "NAVTEX"),
        /// DSC.
        Dsc = ("dsc", "DSC"),
        /// HF weather fax.
        Wefax = ("wefax", "Weather fax"),
    }
}

impl Service {
    /// The service a submode fixes (`navtex`, `dsc`, `wefax`).
    pub fn of_submode(id: &str) -> Option<Service> {
        Service::CHOICES
            .iter()
            .position(|c| c.key == id)
            .and_then(|i| Service::from_index(i as u16))
            .filter(|s| *s != Service::Auto)
    }
}

/// Which decoders run for a service and a dial (0 when unknown): (navtex, dsc_mf, dsc_vhf, fax).
pub fn services(service: Service, dial_hz: f64) -> (bool, bool, bool, bool) {
    let khz = dial_hz / 1000.0;
    let near = |f: f64, tol: f64| (khz - f).abs() <= tol;
    match service {
        Service::Navtex => (true, false, false, false),
        Service::Dsc => {
            if dial_hz <= 0.0 {
                (false, true, true, false)
            } else if dial_hz > 30e6 {
                (false, false, true, false)
            } else {
                (false, true, false, false)
            }
        }
        Service::Wefax => (false, false, false, true),
        Service::Auto => {
            if dial_hz <= 0.0 {
                (true, true, true, true)
            } else if near(490.0, 1.5) || near(518.0, 1.5) || near(4209.5, 1.0) || near(424.0, 1.5)
            {
                (true, false, false, false)
            } else if [2187.5, 4207.5, 6312.0, 8414.5, 12577.0, 16804.5]
                .iter()
                .any(|&f| near(f, 1.0))
            {
                (false, true, false, false)
            } else if near(156_525.0, 10.0) {
                (false, false, true, false)
            } else {
                (false, false, false, true)
            }
        }
    }
}

/// What the decoders tell the display.
#[derive(Clone, Debug)]
pub enum MarineReport {
    /// What the FSK finder and slicer measure.
    Fsk {
        /// The pair found.
        seen: FskSeen,
        /// The symbol rate the slicer measures.
        symbol_rate: f64,
        /// The audio spectrum, (Hz, dB) points.
        spectrum: Vec<[f32; 2]>,
    },
    /// A DSC call.
    Dsc {
        /// The call.
        call: Box<DscCall>,
        /// Heard on VHF channel 70.
        vhf: bool,
        /// The frequency it was heard on, when known.
        freq_hz: Option<f64>,
    },
    /// Whether a decoder holds lock.
    Lock {
        /// Which decoder.
        which: Locker,
        /// It holds lock.
        locked: bool,
    },
    /// A NAVTEX message.
    Navtex(Box<NavtexMessage>),
    /// The fax decoder's status.
    Fax(Box<FaxStatus>),
}
impl Item for MarineReport {
    const KIND: &'static str = "marine.report";
}

/// Soft symbols from a slicer (positive for the higher tone).
#[derive(Clone, Debug, Default)]
pub struct SoftSymbols(pub Vec<f32>);
impl Item for SoftSymbols {
    const KIND: &'static str = "marine.soft";
}

/// The dial: the tuned frequency of a stream, 0 when not known.
fn dial_of(f: &Format) -> f64 {
    f.tuned_hz.max(0.0)
}

/// A stream of items with the tuned frequency of `f`, so the decoders downstream know the dial.
fn items_like<T: Item>(f: &Format) -> Format {
    Format {
        centre_hz: f.centre_hz,
        tuned_hz: f.tuned_hz,
        ..Format::new(Kind::typed::<T>(), 0.0)
    }
}

/// Which decoders run.
pub const SERVICE: Setting<field::Choice<Service>> =
    Setting::choice("service", "Service", Service::Auto)
        .help("Which decoders run. Automatic chooses from the dial.");
/// The name table.
pub const NAMES: Setting<field::Text> = Setting::new("names", "Name table", "", "")
    .help("Ship and coast station names by MMSI to import, one record per line.");

sdrkit_core::settings! {
    /// The settings of [`NavtexComponent`].
    pub enum ServiceSetting {
        /// Which decoders run.
        Service = SERVICE,
    }
}

sdrkit_core::settings! {
    /// The settings of [`DscMfComponent`] and [`VhfDscComponent`].
    pub enum DscSetting {
        /// Which decoders run.
        Service = SERVICE,
        /// The name table.
        Names = NAMES,
    }
}

/// The settings of a DSC decoder applied to its service, names and the text of the names.
fn apply_dsc(
    id: SettingId,
    v: &Value,
    service: &mut Service,
    names: &mut IdNames,
    names_text: &mut String,
) -> Result<Applied, SettingError> {
    match DscSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
        DscSetting::Service => {
            *service = SERVICE.read(id, v)?;
            Ok(Applied::Reformat)
        }
        DscSetting::Names => {
            let t = NAMES.read(id, v)?;
            names.import(t);
            t.clone_into(names_text);
            Ok(Applied::Live)
        }
    }
}

/// The FSK finder and the MF/HF slicer (`marine.fsk`): soft symbols at 100 Bd, and a report at each tick.
pub struct FskComponent {
    finder: FskFinder,
    slicer: MfSlicer,
    soft: Vec<f32>,
    spectrum: Vec<[f32; 2]>,
}

impl Default for FskComponent {
    fn default() -> Self {
        FskComponent {
            finder: FskFinder::new(170.0),
            slicer: MfSlicer::standard(),
            soft: Vec::new(),
            spectrum: Vec::new(),
        }
    }
}

impl Component for FskComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.fsk",
            vec![Port::iq("in")],
            vec![
                Port::typed::<SoftSymbols>("soft").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.finder.reset();
        self.slicer = MfSlicer::standard();
        Ok(vec![
            items_like::<SoftSymbols>(&inputs[0]),
            items_like::<MarineReport>(&inputs[0]),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let t = io.input(0).first() as f64 / CHANNEL_RATE;
        if self.finder.process(x) && self.finder.seen.found {
            self.slicer.set_centre(self.finder.seen.centre_hz);
        }
        self.soft.clear();
        self.slicer.process(x, &mut self.soft);
        if !self.soft.is_empty() {
            io.items_out::<SoftSymbols>(0).push(Timed {
                time: t,
                value: SoftSymbols(self.soft.clone()),
            });
        }
        if io.tick() {
            let mut seen = self.finder.seen;
            if let (true, Some((lo, hi))) = (seen.found, self.slicer.tones()) {
                seen.low_hz = lo;
                seen.high_hz = hi;
                seen.shift_hz = hi - lo;
            }
            self.finder.audio_spectrum(&mut self.spectrum);
            io.items_out::<MarineReport>(1).push(Timed {
                time: t,
                value: MarineReport::Fsk {
                    seen,
                    symbol_rate: self.slicer.symbol_rate(),
                    spectrum: self.spectrum.clone(),
                },
            });
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

/// Both polarities of the DSC decoder with the calls they give, parsed.
struct DscPair {
    dec: [DscDecoder; 2],
}

impl DscPair {
    fn new() -> Self {
        DscPair {
            dec: [DscDecoder::new(false), DscDecoder::new(true)],
        }
    }
    fn reset(&mut self) {
        for d in &mut self.dec {
            d.reset();
        }
    }
    fn locked(&self) -> bool {
        self.dec.iter().any(DscDecoder::locked)
    }
    /// Feeds soft bits (positive for Y, 1) and passes each call worth reporting to `out`: a checked call, or one with a sender.
    fn push(&mut self, soft: &[f32], names: Option<&IdNames>, mut out: impl FnMut(DscCall)) {
        for d in &mut self.dec {
            d.push(soft);
            for raw in d.take_calls() {
                let c = dsccall::parse(&raw, names);
                if c.ecc_ok || !c.from.is_empty() {
                    out(c);
                }
            }
        }
    }
}

/// The decoders that report whether they hold lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locker {
    /// The NAVTEX decoder.
    Navtex,
    /// The MF/HF DSC decoder.
    Dsc,
    /// The VHF channel 70 DSC decoder.
    DscVhf,
    /// The parallel DSC decoders.
    DscParallel,
}

fn lock_report(which: Locker, locked: bool) -> Timed<MarineReport> {
    Timed {
        time: 0.0,
        value: MarineReport::Lock { which, locked },
    }
}

/// The MF/HF DSC decoder (`marine.dsc_mf`): both polarities on the soft symbols.
pub struct DscMfComponent {
    dsc: DscPair,
    service: Service,
    on: bool,
    names: IdNames,
    names_text: String,
    neg: Vec<f32>,
    /// The dial, when known: the frequency the calls are reported on.
    dial_hz: Option<f64>,
}

impl Default for DscMfComponent {
    fn default() -> Self {
        DscMfComponent {
            dsc: DscPair::new(),
            service: Service::Auto,
            on: true,
            names: IdNames::new(IdKind::Mmsi),
            names_text: String::new(),
            neg: Vec::new(),
            dial_hz: None,
        }
    }
}

impl Component for DscMfComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.dsc_mf",
            vec![Port::typed::<SoftSymbols>("soft")],
            vec![Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DscSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.on = services(self.service, dial_of(&inputs[0])).1;
        self.dial_hz = Some(dial_of(&inputs[0])).filter(|f| *f > 0.0);
        self.dsc.reset();
        Ok(vec![Format::new(Kind::typed::<MarineReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.on {
            for s in io.items::<SoftSymbols>(0) {
                // DSC: Y (1) is the lower tone, so the soft value of a 1 is the negative of the slicer's.
                self.neg.clear();
                self.neg.extend(s.value.0.iter().map(|v| -v));
                let (time, freq_hz) = (s.time, self.dial_hz);
                let out = io.items_out::<MarineReport>(0);
                self.dsc.push(&self.neg, Some(&self.names), |c| {
                    out.push(Timed {
                        time,
                        value: MarineReport::Dsc {
                            call: Box::new(c),
                            vhf: false,
                            freq_hz,
                        },
                    })
                });
            }
        }
        if io.tick() {
            let locked = self.on && self.dsc.locked();
            io.items_out::<MarineReport>(0)
                .push(lock_report(Locker::Dsc, locked));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        apply_dsc(
            id,
            v,
            &mut self.service,
            &mut self.names,
            &mut self.names_text,
        )
    }
    fn values(&self) -> Values {
        values_of([
            (DscSetting::Service, Value::Choice(self.service.index())),
            (DscSetting::Names, Value::Text(self.names_text.clone())),
        ])
    }
}

/// The NAVTEX decoder (`marine.navtex`): both polarities, the framer and the text.
pub struct NavtexComponent {
    dec: [SitorDecoder; 2],
    framer: [Framer; 2],
    service: Service,
    on: bool,
}

impl Default for NavtexComponent {
    fn default() -> Self {
        NavtexComponent {
            dec: [SitorDecoder::new(false), SitorDecoder::new(true)],
            framer: Default::default(),
            service: Service::Auto,
            on: true,
        }
    }
}

impl Component for NavtexComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.navtex",
            vec![Port::typed::<SoftSymbols>("soft")],
            vec![Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ServiceSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.on = services(self.service, dial_of(&inputs[0])).0;
        for d in &mut self.dec {
            d.reset();
            d.chars.clear();
        }
        self.framer = Default::default();
        Ok(vec![Format::new(Kind::typed::<MarineReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.on {
            for s in io.items::<SoftSymbols>(0) {
                let out = io.items_out::<MarineReport>(0);
                let mut send = |m: NavtexMessage| {
                    out.push(Timed {
                        time: s.time,
                        value: MarineReport::Navtex(Box::new(m)),
                    })
                };
                for (dec, framer) in self.dec.iter_mut().zip(self.framer.iter_mut()) {
                    dec.push(&s.value.0);
                    for c in dec.chars.drain(..) {
                        if let Some(m) = framer.push(c.ch, c.error) {
                            send(m);
                        }
                    }
                    if std::mem::take(&mut dec.lost) || std::mem::take(&mut dec.idle_timeout) {
                        if let Some(m) = framer.end() {
                            send(m);
                        }
                    }
                }
            }
        }
        if io.tick() {
            let locked = self.on && self.dec.iter().any(SitorDecoder::locked);
            io.items_out::<MarineReport>(0)
                .push(lock_report(Locker::Navtex, locked));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ServiceSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ServiceSetting::Service => self.service = SERVICE.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }
    fn values(&self) -> Values {
        values_of([(ServiceSetting::Service, Value::Choice(self.service.index()))])
    }
}

/// The cut-off of the narrow filter in front of the VHF DSC discriminator (Hz at 24 kHz).
const VHF_DSC_FILTER_HZ: f64 = 4500.0;
/// The largest carrier error the VHF DSC follow-up corrects (Hz).
const VHF_DSC_RANGE_HZ: f64 = 4000.0;
/// The FM deviation that gives a discriminator value of 1 (Hz).
const VHF_DSC_DEVIATION_HZ: f32 = 3000.0;
/// The follow-up takes whole pieces of this many samples, so its mixer steps fall on the same samples however the input is cut.
const VHF_DSC_PIECE: usize = 32;

/// VHF channel 70 DSC (`marine.vhf_dsc`): the discriminator-mean carrier follow-up with its 4.5 kHz filter (3 kHz deviation
/// outputs 1), the 1300 / 2100 Hz AFSK slicer and both polarities.
pub struct VhfDscComponent {
    follow: DiscriminatorFollow,
    /// Samples held until a whole piece of [`VHF_DSC_PIECE`].
    held: Vec<Complex32>,
    slicer: AfskSlicer,
    dsc: DscPair,
    service: Service,
    on: bool,
    audio: Vec<f32>,
    soft: Vec<f32>,
    names: IdNames,
    names_text: String,
    /// The dial, when known.
    dial_hz: Option<f64>,
}

impl Default for VhfDscComponent {
    fn default() -> Self {
        let cfg = DiscFollowConfig {
            range_hz: VHF_DSC_RANGE_HZ,
            ..DiscFollowConfig::new(CHANNEL_RATE, VHF_DSC_FILTER_HZ)
        };
        VhfDscComponent {
            follow: DiscriminatorFollow::new(cfg).expect("the rate, cut-off and range are valid"),
            held: Vec::new(),
            slicer: AfskSlicer::default(),
            dsc: DscPair::new(),
            service: Service::Auto,
            on: true,
            audio: Vec::new(),
            soft: Vec::new(),
            names: IdNames::new(IdKind::Mmsi),
            names_text: String::new(),
            dial_hz: None,
        }
    }
}

impl Component for VhfDscComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.vhf_dsc",
            vec![Port::iq("in").rate(sdrkit_graph::RateRule::Exact(CHANNEL_RATE))],
            vec![Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(DscSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.on = services(self.service, dial_of(&inputs[0])).2;
        self.dial_hz = Some(dial_of(&inputs[0])).filter(|f| *f > 0.0);
        self.follow.reset();
        self.held.clear();
        self.slicer.reset();
        self.dsc.reset();
        Ok(vec![Format::new(Kind::typed::<MarineReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.on {
            let x = io.iq(0);
            let time = (io.input(0).first() as f64 - self.held.len() as f64) / CHANNEL_RATE;
            self.held.extend_from_slice(x);
            let n = self.held.len() - self.held.len() % VHF_DSC_PIECE;
            self.audio.clear();
            self.audio.resize(n, 0.0);
            self.follow
                .process(&self.held[..n], &mut self.audio)
                .expect("the lengths match");
            self.held.drain(..n);
            for a in &mut self.audio {
                *a /= VHF_DSC_DEVIATION_HZ;
            }
            self.soft.clear();
            self.slicer.process(&self.audio, &mut self.soft);
            // Y (1) is 1300 Hz, the lower tone.
            for v in &mut self.soft {
                *v = -*v;
            }
            let freq_hz = self.dial_hz;
            let out = io.items_out::<MarineReport>(0);
            self.dsc.push(&self.soft, Some(&self.names), |mut c| {
                c.vhf = true;
                out.push(Timed {
                    time,
                    value: MarineReport::Dsc {
                        call: Box::new(c),
                        vhf: true,
                        freq_hz,
                    },
                })
            });
        }
        if io.tick() {
            let locked = self.on && self.dsc.locked();
            io.items_out::<MarineReport>(0)
                .push(lock_report(Locker::DscVhf, locked));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        apply_dsc(
            id,
            v,
            &mut self.service,
            &mut self.names,
            &mut self.names_text,
        )
    }
    fn values(&self) -> Values {
        values_of([
            (DscSetting::Service, Value::Choice(self.service.index())),
            (DscSetting::Names, Value::Text(self.names_text.clone())),
        ])
    }
}

/// The USB audio (`marine.usb`): the 24 kHz channel to real audio at 12 kHz.
#[derive(Default)]
pub struct UsbComponent {
    usb: Option<UsbAudio>,
}

impl Component for UsbComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.usb",
            vec![Port::iq("in").rate(sdrkit_graph::RateRule::Exact(CHANNEL_RATE))],
            vec![Port::real("out").rate_from(RateFrom::Fixed(UsbAudio::output_rate(CHANNEL_RATE)))],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.usb = Some(UsbAudio::new(f.rate)?);
        Ok(vec![f
            .with_kind(Kind::Real)
            .with_rate(UsbAudio::output_rate(f.rate))])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        if let Some(u) = &mut self.usb {
            u.process(x, io.real_out(0));
        }
    }
    fn values(&self) -> Values {
        no_values()
    }
}

sdrkit_core::choice_enum! {
    /// The lines per minute of a weather fax.
    pub enum FaxLpm {
        /// Detected from the start tone and the phasing.
        Detect = ("detect", "Detect"),
        /// 60.
        L60 = ("60", "60"),
        /// 90.
        L90 = ("90", "90"),
        /// 120.
        L120 = ("120", "120"),
        /// 240.
        L240 = ("240", "240"),
    }
}

impl FaxLpm {
    /// The lines per minute, 0 to detect.
    pub fn lpm(self) -> u32 {
        match self {
            FaxLpm::Detect => 0,
            FaxLpm::L60 => 60,
            FaxLpm::L90 => 90,
            FaxLpm::L120 => 120,
            FaxLpm::L240 => 240,
        }
    }
}

sdrkit_core::choice_enum! {
    /// The index of cooperation of a weather fax.
    pub enum FaxIoc {
        /// From the start tone.
        Tone = ("tone", "From the start tone"),
        /// 576.
        I576 = ("576", "576"),
        /// 288.
        I288 = ("288", "288"),
    }
}

impl FaxIoc {
    /// The index of cooperation, 0 to take it from the start tone.
    pub fn ioc(self) -> u32 {
        match self {
            FaxIoc::Tone => 0,
            FaxIoc::I576 => 576,
            FaxIoc::I288 => 288,
        }
    }
}

sdrkit_core::choice_enum! {
    /// How far a fax picture is turned.
    pub enum FaxRotate {
        /// Not turned.
        None = ("0", "None"),
        /// One quarter turn.
        One = ("1", "1 turn"),
        /// Two quarter turns.
        Two = ("2", "2 turns"),
        /// Three quarter turns.
        Three = ("3", "3 turns"),
        /// One quarter turn back.
        Back = ("-1", "1 turn back"),
    }
}

impl FaxRotate {
    /// The quarter turns, negative turns back.
    pub fn turns(self) -> i32 {
        match self {
            FaxRotate::None => 0,
            FaxRotate::One => 1,
            FaxRotate::Two => 2,
            FaxRotate::Three => 3,
            FaxRotate::Back => -1,
        }
    }
}

/// Lines per minute.
pub const FAX_LPM: Setting<field::Choice<FaxLpm>> =
    Setting::choice("fax_lpm", "Lines per minute", FaxLpm::Detect).section("Weather fax");
/// The index of cooperation.
pub const FAX_IOC: Setting<field::Choice<FaxIoc>> =
    Setting::choice("fax_ioc", "Index of cooperation", FaxIoc::Tone).section("Weather fax");
/// Automatic slant.
pub const FAX_AUTO_SLANT: Setting<field::Bool> =
    Setting::new("fax_auto_slant", "Automatic slant", "", true).section("Weather fax");
/// The slant.
pub const FAX_SLANT_PPM: Setting<field::F64> = Setting::new("fax_slant_ppm", "Slant", "ppm", 0.0)
    .range(-300.0, 300.0, 0.1)
    .section("Weather fax");
/// The most lines of a picture.
pub const FAX_MAX_LINES: Setting<field::I64> =
    Setting::new("fax_max_lines", "Most lines", "", 8000)
        .range(1.0, MAX_LINES as f64, 1.0)
        .section("Weather fax");
/// The row filter.
pub const FAX_ROW_FILTER: Setting<field::Bool> = Setting::new("row_filter", "Row filter", "", true)
    .help("Smooths the noise along each row of a weak fax picture, with the noise measured on the row.")
    .section("Weather fax");
/// The rotation.
pub const FAX_ROTATE: Setting<field::Choice<FaxRotate>> =
    Setting::choice("fax_rotate", "Rotation", FaxRotate::None).section("Weather fax");

sdrkit_core::settings! {
    /// The settings of [`FaxComponent`].
    pub enum FaxSetting {
        /// Which decoders run.
        Service = SERVICE,
        /// Lines per minute.
        Lpm = FAX_LPM,
        /// The index of cooperation.
        Ioc = FAX_IOC,
        /// Automatic slant.
        AutoSlant = FAX_AUTO_SLANT,
        /// The slant.
        SlantPpm = FAX_SLANT_PPM,
        /// The most lines of a picture.
        MaxLines = FAX_MAX_LINES,
        /// The rotation.
        Rotate = FAX_ROTATE,
        /// The row filter.
        RowFilter = FAX_ROW_FILTER,
    }
}

/// Clears the fax picture.
pub const CLEAR_PICTURE: Command =
    Command::new("clear_picture", "Clear the picture").help("Starts the fax picture again.");

sdrkit_core::commands! {
    /// The commands of [`FaxComponent`].
    pub enum FaxCommand {
        /// Clear the picture.
        ClearPicture = CLEAR_PICTURE,
    }
}

/// The fax decoder (`marine.fax`): the status for the display and the picture as grey pixels on the picture stream.
pub struct FaxComponent {
    dec: Option<FaxDecoder>,
    service: Service,
    on: bool,
    settings: FaxSettings,
    lpm: FaxLpm,
    ioc: FaxIoc,
    rotate: FaxRotate,
    last_change: Option<u64>,
    clear: bool,
}

impl Default for FaxComponent {
    fn default() -> Self {
        FaxComponent {
            dec: None,
            service: Service::Auto,
            on: true,
            settings: FaxSettings::default(),
            lpm: FaxLpm::Detect,
            ioc: FaxIoc::Tone,
            rotate: FaxRotate::None,
            last_change: None,
            clear: false,
        }
    }
}

impl Component for FaxComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.fax",
            vec![Port::real("in")],
            vec![
                Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Picture>("picture").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(FaxSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let f = &inputs[0];
        self.on = services(self.service, dial_of(f)).3;
        let mut d = FaxDecoder::new(f.rate)?;
        d.settings = self.settings.clone();
        if let Some(old) = &mut self.dec {
            d.carry_pictures(old);
        }
        self.dec = Some(d);
        self.last_change = None;
        Ok(vec![
            items_like::<MarineReport>(f),
            Format::new(Kind::typed::<Picture>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let Some(d) = &mut self.dec else {
            return;
        };
        if std::mem::take(&mut self.clear) {
            d.clear();
        }
        if self.on {
            d.process(io.real(0));
        }
        if io.tick() {
            let t = io.input(0).end() as f64 / io.input_format(0).rate.max(1.0);
            if self.on {
                io.items_out::<MarineReport>(0).push(Timed {
                    time: t,
                    value: MarineReport::Fax(Box::new(d.status())),
                });
            }
            for p in d.take_ended() {
                io.items_out::<Picture>(1).push(Timed { time: t, value: p });
            }
            if let Some((change, p)) = d.picture() {
                if self.last_change != Some(change) {
                    self.last_change = Some(change);
                    io.items_out::<Picture>(1).push(Timed { time: t, value: p });
                }
            }
        }
    }
    fn finish(&mut self, io: &mut Io) {
        self.process(io);
        let Some(d) = &mut self.dec else {
            return;
        };
        // The end of the input ends the page.
        d.end_page();
        let t = io.input(0).end() as f64 / io.input_format(0).rate.max(1.0);
        for p in d.take_ended() {
            io.items_out::<Picture>(1).push(Timed { time: t, value: p });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let s = &mut self.settings;
        match FaxSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            FaxSetting::Service => {
                self.service = SERVICE.read(id, v)?;
                return Ok(Applied::Reformat);
            }
            FaxSetting::Lpm => {
                self.lpm = FAX_LPM.read(id, v)?;
                s.lpm = self.lpm.lpm();
            }
            FaxSetting::Ioc => {
                self.ioc = FAX_IOC.read(id, v)?;
                s.ioc = self.ioc.ioc();
            }
            FaxSetting::AutoSlant => s.auto_slant = FAX_AUTO_SLANT.read(id, v)?,
            FaxSetting::SlantPpm => s.slant_ppm = FAX_SLANT_PPM.read(id, v)?,
            FaxSetting::MaxLines => s.max_lines = FAX_MAX_LINES.read(id, v)? as usize,
            FaxSetting::Rotate => {
                self.rotate = FAX_ROTATE.read(id, v)?;
                s.rotate = self.rotate.turns();
            }
            FaxSetting::RowFilter => s.row_filter = FAX_ROW_FILTER.read(id, v)?,
        }
        if let Some(d) = &mut self.dec {
            let moved = d.settings.slant_ppm != s.slant_ppm || d.settings.rotate != s.rotate;
            d.settings = s.clone();
            if moved {
                d.bump_change();
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        let s = &self.settings;
        values_of([
            (FaxSetting::Service, Value::Choice(self.service.index())),
            (FaxSetting::Lpm, Value::Choice(self.lpm.index())),
            (FaxSetting::Ioc, Value::Choice(self.ioc.index())),
            (FaxSetting::AutoSlant, Value::Bool(s.auto_slant)),
            (FaxSetting::SlantPpm, Value::F64(s.slant_ppm)),
            (FaxSetting::MaxLines, Value::I64(s.max_lines as i64)),
            (FaxSetting::Rotate, Value::Choice(self.rotate.index())),
            (FaxSetting::RowFilter, Value::Bool(s.row_filter)),
        ])
    }
    fn commands(&self) -> &Commands {
        FaxCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match FaxCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            FaxCommand::ClearPicture => self.clear = true,
        }
        Ok(None)
    }
}

/// Listen to the sideband.
pub const LISTEN: Setting<field::Bool> = Setting::new("listen", "Listen", "", false);

sdrkit_core::settings! {
    /// The settings of [`ListenComponent`].
    pub enum ListenSetting {
        /// Listen to the sideband.
        Listen = LISTEN,
    }
}

/// Listen (`marine.listen`): the sideband from the dial on the audio output (mono).
#[derive(Default)]
pub struct ListenComponent {
    on: bool,
}

impl Component for ListenComponent {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.listen",
            vec![Port::real("in")],
            vec![Port::audio("out").rate_from(RateFrom::Input(0))],
        )
        .with_settings(ListenSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_kind(Kind::Audio)])
    }
    fn process(&mut self, io: &mut Io) {
        if !self.on {
            return;
        }
        let x = io.real(0);
        io.audio_out(0).samples.extend_from_slice(x);
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ListenSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ListenSetting::Listen => self.on = LISTEN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(ListenSetting::Listen, Value::Bool(self.on))])
    }
}

/// Parallel DSC.
pub const DSC_PARALLEL: Setting<field::Bool> =
    Setting::new("dsc_parallel", "Parallel DSC", "", false).section("DSC");
/// The DSC channel sets.
pub const DSC_SCAN_SETS: Setting<field::Text> =
    Setting::new("dsc_scan_sets", "DSC channel sets", "", "distress")
        .help("distress, worldwide, series_a, series_b, vhf")
        .section("DSC");

sdrkit_core::settings! {
    /// The settings of [`ParallelDsc`].
    pub enum ParallelSetting {
        /// Parallel DSC.
        Parallel = DSC_PARALLEL,
        /// The DSC channel sets.
        Sets = DSC_SCAN_SETS,
    }
}

/// One parallel DSC channel: its front end, finder, slicer and decoders.
struct Chain {
    hz: f64,
    fe: FixedRateFrontEnd,
    finder: FskFinder,
    slicer: MfSlicer,
    dsc: DscPair,
}

/// Parallel DSC (`marine.parallel`): every chosen MF/HF channel inside the capture (8 kHz margin, more than 500 Hz from the dial)
/// gets its own front end, slicer and decoders.
pub struct ParallelDsc {
    on: bool,
    sets: Vec<&'static str>,
    sets_text: String,
    chains: Vec<Chain>,
    rate: f64,
    input: Option<Format>,
    chan: Vec<Complex32>,
    soft: Vec<f32>,
}

impl Default for ParallelDsc {
    fn default() -> Self {
        ParallelDsc {
            on: false,
            sets: vec!["distress"],
            sets_text: "distress".into(),
            chains: Vec::new(),
            rate: 0.0,
            input: None,
            chan: Vec::new(),
            soft: Vec::new(),
        }
    }
}

impl ParallelDsc {
    fn build(&mut self) -> Result<(), ConfigError> {
        self.chains.clear();
        let Some(f) = self.input.clone() else {
            return Ok(());
        };
        let dial = f.tuned_hz;
        if !self.on || dial <= 0.0 {
            return Ok(());
        }
        let dial_khz = dial / 1000.0;
        let list: Vec<f64> = channels::scan_set(&self.sets)
            .iter()
            .map(|c| c.khz)
            .filter(|k| *k < 30_000.0)
            .collect();
        let offs = f.signal_offset_hz();
        for khz in channels::parallel_channels(&list, dial_khz, f.rate, offs) {
            let mut fe = FixedRateFrontEnd::new((khz - dial_khz) * 1000.0, 10e3, CHANNEL_RATE)
                .map_err(ConfigError::Setting)?;
            fe.configure(std::slice::from_ref(&f))?;
            self.chains.push(Chain {
                hz: khz * 1000.0,
                fe,
                finder: FskFinder::new(170.0),
                slicer: MfSlicer::standard(),
                dsc: DscPair::new(),
            });
        }
        self.rate = f.rate;
        Ok(())
    }
    /// The channels decoded in parallel (Hz).
    pub fn channels(&self) -> Vec<f64> {
        self.chains.iter().map(|c| c.hz).collect()
    }
}

impl Component for ParallelDsc {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "marine.parallel",
            vec![Port::iq("in")],
            vec![Port::typed::<MarineReport>("report").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(ParallelSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.input = Some(inputs[0].clone());
        self.build()?;
        Ok(vec![Format::new(Kind::typed::<MarineReport>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        if self.chains.is_empty() {
            return;
        }
        let x = io.iq(0);
        let time = io.input(0).first() as f64 / self.rate.max(1.0);
        let out = io.items_out::<MarineReport>(0);
        for c in &mut self.chains {
            self.chan.clear();
            c.fe.process_slice(x, &mut self.chan);
            if c.finder.process(&self.chan) && c.finder.seen.found {
                c.slicer.set_centre(c.finder.seen.centre_hz);
            }
            self.soft.clear();
            c.slicer.process(&self.chan, &mut self.soft);
            for v in &mut self.soft {
                *v = -*v;
            }
            let freq_hz = Some(c.hz);
            c.dsc.push(&self.soft, None, |call| {
                out.push(Timed {
                    time,
                    value: MarineReport::Dsc {
                        call: Box::new(call),
                        vhf: false,
                        freq_hz,
                    },
                })
            });
        }
        if io.tick() {
            let locked = self.chains.iter().any(|c| c.dsc.locked());
            io.items_out::<MarineReport>(0)
                .push(lock_report(Locker::DscParallel, locked));
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ParallelSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ParallelSetting::Parallel => self.on = DSC_PARALLEL.read(id, v)?,
            ParallelSetting::Sets => {
                let t = DSC_SCAN_SETS.read(id, v)?;
                self.sets = scan_sets(id, t)?;
                t.clone_into(&mut self.sets_text);
            }
        }
        self.build().map_err(|e| SettingError::Config {
            id,
            source: Box::new(e),
        })?;
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (ParallelSetting::Parallel, Value::Bool(self.on)),
            (ParallelSetting::Sets, Value::Text(self.sets_text.clone())),
        ])
    }
}

/// The scan sets that the text of setting `id` names.
///
/// # Errors
///
/// [`SettingError::Malformed`] if it names none.
fn scan_sets(id: SettingId, text: &str) -> Result<Vec<&'static str>, SettingError> {
    let sets = channels::parse_scan_sets(text);
    if sets.is_empty() {
        return Err(SettingError::Malformed { id, at: None });
    }
    Ok(sets)
}

// ---------------------------------------------------------------------------------------------------------------------------------
// Fields, tables and events.

sdrkit_core::choice_enum! {
    /// The service heard last.
    pub enum Active {
        /// None.
        None = ("none", "None"),
        /// NAVTEX.
        Navtex = ("navtex", "NAVTEX"),
        /// MF/HF DSC.
        Dsc = ("dsc", "DSC"),
        /// VHF DSC.
        DscVhf = ("dsc_vhf", "DSC VHF"),
        /// Weather fax.
        Fax = ("fax", "Weather fax"),
    }
}

sdrkit_core::choice_enum! {
    /// The state of the fax decoder.
    pub enum FaxState {
        /// Waiting for a start tone.
        Waiting = ("waiting", "Waiting"),
        /// Phasing.
        Phasing = ("phasing", "Phasing"),
        /// Receiving a picture.
        Receiving = ("receiving", "Receiving"),
        /// Stopped.
        Stopped = ("stopped", "Stopped"),
    }
}

/// The prefix of the map points of DSC positions.
pub const POINT_PREFIX: &str = "dsc:";
/// The key of the series of the audio spectrum.
pub const SPECTRUM_SERIES: &str = "audio_spectrum";

/// The result fields of the marine HF stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct HfFields<'a> {
    /// The reception state of a display.
    #[field(label = "State")]
    pub state: Hearing,
    /// The service heard last.
    #[field(label = "Service")]
    pub active_service: Active,
    /// An FSK pair stands above the noise.
    #[field(label = "FSK pair")]
    pub fsk_found: bool,
    /// The lower FSK tone, from the dial.
    #[field(label = "Low tone", unit = "Hz")]
    pub fsk_low_hz: Option<Hertz>,
    /// The higher FSK tone, from the dial.
    #[field(label = "High tone", unit = "Hz")]
    pub fsk_high_hz: Option<Hertz>,
    /// The measured shift.
    #[field(label = "Shift", unit = "Hz")]
    pub shift_hz: Option<Hertz>,
    /// Symbol rate found, Hz.
    #[field(shared = keys::SYMBOL_RATE_HZ)]
    pub symbol_rate_hz: Option<Hertz>,
    /// The level of the tones over the noise, per bin.
    #[field(label = "Tones over the noise", unit = "dB")]
    pub tone_level_db: Option<f64>,
    /// Signal to noise ratio of what is being decoded, dB (in the signal's own bandwidth, or per symbol where the stack says so).
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// Carrier offset from where it was expected, Hz.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// NAVTEX messages complete with no unreadable character.
    #[field(label = "NAVTEX good")]
    pub navtex_good: i64,
    /// Other NAVTEX messages.
    #[field(label = "NAVTEX bad")]
    pub navtex_bad: i64,
    /// DSC calls with a good check.
    #[field(label = "DSC good")]
    pub dsc_good: i64,
    /// DSC calls with a failed check.
    #[field(label = "DSC bad")]
    pub dsc_bad: i64,
    /// The DSC list holds a distress call.
    #[field(label = "Distress")]
    pub distress: bool,
    /// The fax decoder's state.
    #[field(label = "Fax")]
    pub fax_state: Option<FaxState>,
    /// The index of cooperation.
    #[field(label = "IOC")]
    pub ioc: Option<i64>,
    /// Lines per minute.
    #[field(label = "Lines per minute")]
    pub lpm: Option<i64>,
    /// Picture lines received.
    #[field(label = "Lines")]
    pub lines: Option<i64>,
    /// Picture width, pixels.
    #[field(label = "Width")]
    pub width: Option<i64>,
    /// The slant in use.
    #[field(label = "Slant", unit = "ppm")]
    pub slant_ppm: Option<f64>,
    /// The black level.
    #[field(label = "Black level", unit = "Hz")]
    pub black_level_hz: Option<Hertz>,
    /// The last start or stop tone measured.
    #[field(label = "Last tone", unit = "Hz")]
    pub last_tone_hz: Option<Hertz>,
    /// Phasing lines seen.
    #[field(label = "Phasing lines")]
    pub phasing_lines: Option<i64>,
    /// The fax SNR: tone over audio power, or the black-white swing over the noise of the black level.
    #[field(label = "Fax SNR", unit = "dB")]
    pub fax_snr_db: Option<f64>,
    /// Pictures started.
    #[field(label = "Pictures started")]
    pub pictures_started: Option<i64>,
    /// Pictures finished.
    #[field(label = "Pictures done")]
    pub pictures_done: Option<i64>,
    /// Why the last picture ended.
    #[field(label = "Picture end")]
    pub end_reason: Option<&'a str>,
    /// The channel the scanner holds.
    #[field(label = "Scan channel", unit = "Hz")]
    pub scan_channel_hz: Option<Hertz>,
    /// Reports made.
    #[field(label = "Reports")]
    pub reports: i64,
    /// Signal time since the start.
    #[field(label = "Signal time", unit = "s")]
    pub signal_time: Seconds,
}

/// A row of the NAVTEX message table. The row key is the place of the message in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NavtexRow<'a> {
    /// When a message or call was heard (wall time).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// B1 B2 B3 B4.
    #[field(label = "Header")]
    pub header: &'a str,
    /// The name of the subject.
    #[field(label = "Subject")]
    pub subject_name: &'a str,
    /// Characters of the text.
    #[field(label = "Characters")]
    pub chars: i64,
    /// Unreadable characters.
    #[field(label = "Unreadable")]
    pub errors: i64,
    /// NNNN was seen.
    #[field(label = "Complete")]
    pub complete: bool,
    /// Times the message came again.
    #[field(label = "Repeats")]
    pub repeats: i64,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: &'a str,
}

/// The NAVTEX messages, newest first.
pub static NAVTEX_TABLE: Table<NavtexRow<'static>> =
    Table::new("navtex", "NAVTEX", crate::navtex::LIST_CAP)
        .role(sdrkit_core::TableRole::Messages)
        .label(&NavtexRow::HEADER);

/// A row of the DSC call table. The row key is the place of the call in the log.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DscRow<'a> {
    /// When a message or call was heard (wall time).
    #[field(label = "Time")]
    pub time: Option<Time>,
    /// The frequency a station was heard on.
    #[field(shared = keys::FREQ_HZ)]
    pub freq_hz: Option<Hertz>,
    /// The kind of call in words.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The sender's MMSI.
    #[field(label = "From", role = Sender)]
    pub from_mmsi: Option<&'a str>,
    /// The sender's name.
    #[field(label = "Name")]
    pub from_name: Option<&'a str>,
    /// The sender's country.
    #[field(label = "Country")]
    pub from_country: Option<&'a str>,
    /// The called station.
    #[field(label = "To", role = Addressee)]
    pub to_mmsi: Option<&'a str>,
    /// The category in words.
    #[field(label = "Category")]
    pub category_name: Option<&'a str>,
    /// The nature of distress in words.
    #[field(label = "Nature")]
    pub nature_name: Option<&'a str>,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The telecommands in words.
    #[field(label = "Telecommands")]
    pub telecommands: Option<&'a str>,
    /// The receive frequency or channel the call proposes.
    #[field(label = "RX frequency")]
    pub freq_rx: Option<&'a str>,
    /// The transmit frequency or channel the call proposes.
    #[field(label = "TX frequency")]
    pub freq_tx: Option<&'a str>,
    /// The error-check character matched.
    #[field(label = "Check")]
    pub ecc_ok: bool,
    /// Symbols taken from the second copy or rebuilt.
    #[field(label = "Repaired")]
    pub repaired: i64,
    /// Symbols unreadable in both copies.
    #[field(label = "Erasures")]
    pub erasures: i64,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT, role = Text)]
    pub text: &'a str,
}

/// The DSC calls, newest first.
pub static DSC_TABLE: Table<DscRow<'static>> = Table::new("dsc", "DSC", DSC_CAP)
    .role(sdrkit_core::TableRole::Calls)
    .label(&DscRow::FROM_MMSI);

/// The fields of the event of a DSC call.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DscCallEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The receiver of a message or call: an address, a call sign or a talk group member.
    #[field(shared = keys::TARGET)]
    pub target: Option<&'a str>,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The position of the object that the event is about.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The format specifier.
    #[field(label = "Format")]
    pub format: i64,
    /// The kind of call in words.
    #[field(label = "Kind")]
    pub kind: &'a str,
    /// The category code.
    #[field(label = "Category code")]
    pub category: Option<i64>,
    /// The category in words.
    #[field(label = "Category")]
    pub category_name: Option<&'a str>,
    /// The sender's name.
    #[field(label = "Name")]
    pub from_name: Option<&'a str>,
    /// The sender's country.
    #[field(label = "Country")]
    pub from_country: Option<&'a str>,
    /// What the called station is.
    #[field(label = "Called station")]
    pub to_type: Option<&'a str>,
    /// The station in distress.
    #[field(label = "In distress")]
    pub distress_mmsi: Option<&'a str>,
    /// The nature of distress code.
    #[field(label = "Nature code")]
    pub nature: Option<i64>,
    /// The nature of distress in words.
    #[field(label = "Nature")]
    pub nature_name: Option<&'a str>,
    /// The position in the call, as sent.
    #[field(label = "Position text")]
    pub position_text: Option<&'a str>,
    /// The time in the call (HH:MM UTC).
    #[field(label = "UTC")]
    pub utc: Option<&'a str>,
    /// The first telecommand.
    #[field(label = "Telecommand 1")]
    pub telecommand1: Option<i64>,
    /// The second telecommand.
    #[field(label = "Telecommand 2")]
    pub telecommand2: Option<i64>,
    /// The telecommands in words.
    #[field(label = "Telecommands")]
    pub telecommands: Option<&'a str>,
    /// The receive frequency or channel the call proposes.
    #[field(label = "RX frequency")]
    pub freq_rx: &'a str,
    /// The transmit frequency or channel the call proposes.
    #[field(label = "TX frequency")]
    pub freq_tx: &'a str,
    /// The end of sequence character.
    #[field(label = "End of sequence")]
    pub eos: Option<i64>,
    /// The error-check character matched.
    #[field(label = "Check")]
    pub ecc_ok: bool,
    /// Symbols taken from the second copy or rebuilt.
    #[field(label = "Repaired")]
    pub repaired: i64,
    /// Symbols unreadable in both copies.
    #[field(label = "Erasures")]
    pub erasures: i64,
    /// Heard on VHF channel 70.
    #[field(label = "VHF")]
    pub vhf: bool,
}

/// A DSC call was decoded.
pub static DSC_CALL: EventKind<DscCallEvent<'static>> = EventKind::new("dsc.call", "DSC call");

/// The fields of the event of a NAVTEX message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NavtexMessageEvent<'a> {
    /// The sender of a decoded message: a registration, an address, a call sign or a code.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What an event is about (aircraft, vessel, pager). Outputs and the store filter on it.
    #[field(shared = keys::FAMILY)]
    pub family: Family,
    /// How urgent an event is. A stack sets it when the decoded message is urgent. Services alert on it.
    #[field(shared = keys::URGENCY)]
    pub urgency: Urgency,
    /// The text of a decoded message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// B1 B2 B3 B4.
    #[field(label = "Header")]
    pub header: &'a str,
    /// B2, the subject indicator.
    #[field(label = "Subject")]
    pub subject: &'a str,
    /// The name of the subject.
    #[field(label = "Subject")]
    pub subject_name: &'a str,
    /// B3 B4, the serial number.
    #[field(label = "Number")]
    pub number: i64,
    /// NNNN was seen.
    #[field(label = "Complete")]
    pub complete: bool,
    /// Unreadable characters.
    #[field(label = "Unreadable")]
    pub errors: i64,
    /// Characters of the text.
    #[field(label = "Characters")]
    pub chars: i64,
}

/// A NAVTEX message ended.
pub static NAVTEX_MESSAGE: EventKind<NavtexMessageEvent<'static>> =
    EventKind::new("navtex.message", "NAVTEX message");

// ---------------------------------------------------------------------------------------------------------------------------------
// The lists the display keeps.

/// One DSC call in the list.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DscEntry {
    /// When it was heard.
    pub at: Stamp,
    /// The frequency it was heard on, when known.
    pub freq_hz: Option<f64>,
    /// The call.
    pub call: DscCall,
}

impl Shift for DscEntry {
    fn shift_times(&mut self, by_s: f64) {
        self.at.shift_times(by_s);
    }
}

/// The NAVTEX and DSC lists with their totals: what the display keeps across sessions (format `marine.hf_lists`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HfLists {
    /// The NAVTEX messages and their totals.
    pub navtex: MessageList,
    /// The DSC calls, oldest first.
    pub dsc: LogRing<DscEntry>,
    /// DSC calls with a good check since the start.
    pub dsc_good: u64,
    /// DSC calls with a failed check since the start.
    pub dsc_bad: u64,
}

impl Default for HfLists {
    fn default() -> Self {
        HfLists {
            navtex: MessageList::new(),
            dsc: LogRing::new(DSC_CAP),
            dsc_good: 0,
            dsc_bad: 0,
        }
    }
}

impl Shift for HfLists {
    fn shift_times(&mut self, by_s: f64) {
        self.navtex.shift_times(by_s);
        self.dsc.shift_times(by_s);
    }
}

impl Tables for HfLists {
    const FORMAT: StateFormat = StateFormat::new("marine.hf_lists", 1);
    fn clear(&mut self) {
        self.navtex.clear();
        self.dsc.clear();
        self.dsc_good = 0;
        self.dsc_bad = 0;
    }
    fn adopt(&mut self, saved: Self) {
        self.navtex.messages.adopt(saved.navtex.messages);
        self.navtex.good = saved.navtex.good;
        self.navtex.bad = saved.navtex.bad;
        self.dsc.adopt(saved.dsc);
        self.dsc_good = saved.dsc_good;
        self.dsc_bad = saved.dsc_bad;
    }
}

/// `s`, or `None` when it is empty.
fn some(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

impl Shaped for NavtexMessage {
    type Row = NavtexRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NavtexRow {
            time: self.at.wall.map(Time),
            header: &self.header,
            subject_name: &self.subject_name,
            chars: self.chars as i64,
            errors: self.errors as i64,
            complete: self.complete,
            repeats: i64::from(self.repeats),
            text: &self.text,
        });
    }
}

impl Shaped for DscEntry {
    type Row = DscRow<'static>;
    fn row(&self, _: Option<&Seen>, _: f64, out: RowOut<'_, '_, Self::Row>) {
        let c = &self.call;
        out.put(&DscRow {
            time: self.at.wall.map(Time),
            freq_hz: self.freq_hz.map(Hertz),
            kind: &c.kind,
            from_mmsi: some(&c.from),
            from_name: some(&c.from_name),
            from_country: some(&c.from_country),
            to_mmsi: some(&c.to),
            category_name: some(&c.category_name),
            nature_name: some(&c.nature_name),
            position: c.lat.zip(c.lon),
            telecommands: some(&c.telecommands),
            freq_rx: some(&c.freq_rx),
            freq_tx: some(&c.freq_tx),
            ecc_ok: c.ecc_ok,
            repaired: c.repaired as i64,
            erasures: c.erasures as i64,
            text: &c.text,
        });
    }
}

/// The urgency of a DSC call (ITU-R M.493-15 Table A1-3: category 112 is distress, 110 urgency).
pub fn dsc_urgency(c: &DscCall) -> Urgency {
    if c.distress || c.category == Some(112) {
        Urgency::Emergency
    } else if c.category == Some(110) {
        Urgency::Urgent
    } else {
        Urgency::None
    }
}

/// The urgency of a NAVTEX message (IMO NAVTEX Manual, subject indicator B2: B is a meteorological warning, D search and
/// rescue).
pub fn navtex_urgency(m: &NavtexMessage) -> Urgency {
    match m.subject {
        'B' => Urgency::Notice,
        'D' => Urgency::Urgent,
        _ => Urgency::None,
    }
}

fn dsc_event(c: &DscCall, t: f64, freq_hz: f64) -> Event {
    let code = |v: Option<u8>| v.map(i64::from);
    DSC_CALL.event(
        t,
        freq_hz,
        &DscCallEvent {
            source: some(&c.from),
            target: some(&c.to),
            family: Family::Vessel,
            urgency: dsc_urgency(c),
            text: &c.text,
            position: c.lat.zip(c.lon),
            format: i64::from(c.format),
            kind: &c.kind,
            category: code(c.category),
            category_name: some(&c.category_name),
            from_name: some(&c.from_name),
            from_country: some(&c.from_country),
            to_type: some(&c.to_type),
            distress_mmsi: some(&c.distress_mmsi),
            nature: code(c.nature),
            nature_name: some(&c.nature_name),
            position_text: some(&c.position_text),
            utc: some(&c.utc),
            telecommand1: code(c.telecommand1),
            telecommand2: code(c.telecommand2),
            telecommands: some(&c.telecommands),
            freq_rx: &c.freq_rx,
            freq_tx: &c.freq_tx,
            eos: code(c.eos),
            ecc_ok: c.ecc_ok,
            repaired: c.repaired as i64,
            erasures: c.erasures as i64,
            vhf: c.vhf,
        },
    )
}

fn navtex_event(m: &NavtexMessage, t: f64, freq_hz: f64) -> Event {
    let mut b = [0u8; 4];
    NAVTEX_MESSAGE.event(
        t,
        freq_hz,
        &NavtexMessageEvent {
            source: m.station.encode_utf8(&mut b),
            family: Family::Vessel,
            urgency: navtex_urgency(m),
            text: &m.text,
            header: &m.header,
            subject: m.subject.encode_utf8(&mut [0u8; 4]),
            subject_name: &m.subject_name,
            number: i64::from(m.number),
            complete: m.complete,
            errors: m.errors as i64,
            chars: m.chars as i64,
        },
    )
}

// ---------------------------------------------------------------------------------------------------------------------------------
// The display.

/// The results of the marine HF receiver (`marine.display`): the lists, the measurements, the scanner and its retune requests.
pub struct MarineDisplay {
    p: Published,
    lists: HfLists,
    clock: Clock,
    host_clock: sdrkit_core::Clock,
    live: bool,
    fsk: (FskSeen, f64),
    spectrum: Vec<[f32; 2]>,
    /// Whether each decoder holds lock, by [`Locker`].
    locks: [bool; 4],
    fax: Option<FaxStatus>,
    last_navtex: f64,
    last_dsc: f64,
    last_fax: f64,
    reports: u64,
    service: Service,
    dial_hz: f64,
    flags: (bool, bool, bool, bool),
    scanner: Option<Scanner>,
    scan: bool,
    scan_sets: Vec<&'static str>,
    scan_sets_text: String,
    scan_dwell: f64,
    time_base: f64,
    /// The scanner's clock: report periods counted, so it runs on through the retunes the scanner itself requests (the signal
    /// clock starts again at each).
    scan_clock: f64,
    /// Parallel decoding is on (it excludes the scanner).
    parallel: bool,
    /// The channel the scanner last requested (Hz).
    scan_asked: Option<f64>,
    pending_events: Vec<Event>,
    pending_requests: Vec<HostRequest>,
    items: Vec<Timed<MarineReport>>,
}

impl Default for MarineDisplay {
    fn default() -> Self {
        let mut p = Published::from_decl(Arc::new(crate::typed::shape("marine").results_decl()));
        p.results_mut().set_status(&Status {
            lock_state: Hearing::Searching.lock_state(),
            summary: "Marine: searching".into(),
            ..Status::default()
        });
        MarineDisplay {
            p,
            lists: HfLists::default(),
            clock: Clock::default(),
            host_clock: sdrkit_core::Clock::fixed(0.0),
            live: false,
            fsk: (FskSeen::default(), 0.0),
            spectrum: Vec::new(),
            locks: [false; 4],
            fax: None,
            last_navtex: f64::NEG_INFINITY,
            last_dsc: f64::NEG_INFINITY,
            last_fax: f64::NEG_INFINITY,
            reports: 0,
            service: Service::Auto,
            dial_hz: 0.0,
            flags: (true, true, true, true),
            scanner: None,
            scan: false,
            scan_sets: vec!["distress"],
            scan_sets_text: "distress".into(),
            scan_dwell: 4.0,
            time_base: 0.0,
            scan_clock: 0.0,
            parallel: false,
            scan_asked: None,
            pending_events: Vec::new(),
            pending_requests: Vec::new(),
            items: Vec::new(),
        }
    }
}

impl MarineDisplay {
    /// The NAVTEX and DSC lists.
    pub fn lists(&self) -> &HfLists {
        &self.lists
    }

    fn on_report(&mut self, t: f64, r: &MarineReport) {
        match r {
            MarineReport::Fsk {
                seen,
                symbol_rate,
                spectrum,
            } => {
                self.fsk = (*seen, *symbol_rate);
                self.spectrum.clone_from(spectrum);
            }
            MarineReport::Lock { which, locked } => {
                self.locks[*which as usize] = *locked;
            }
            MarineReport::Dsc {
                call,
                vhf: _,
                freq_hz,
            } => {
                if call.ecc_ok {
                    self.lists.dsc_good += 1;
                    self.last_dsc = t;
                } else {
                    self.lists.dsc_bad += 1;
                }
                let freq = freq_hz.or_else(|| (self.dial_hz > 0.0).then_some(self.dial_hz));
                self.p.results_mut().log(format!("DSC {}", call.text));
                if let Some(s) = &mut self.scanner {
                    s.call(self.scan_clock);
                }
                self.pending_events
                    .push(dsc_event(call, t, freq.unwrap_or(0.0)));
                self.lists.dsc.push(DscEntry {
                    at: self.clock.stamp(t),
                    freq_hz: freq,
                    call: (**call).clone(),
                });
            }
            MarineReport::Navtex(m) => {
                let mut m = (**m).clone();
                m.at = self.clock.stamp(t);
                m.freq_hz = (self.dial_hz > 0.0).then_some(self.dial_hz);
                self.last_navtex = t;
                if self.lists.navtex.add(m.clone()) {
                    self.p.results_mut().log(format!(
                        "NAVTEX {}{}: {}, {} characters, {} unreadable",
                        m.header,
                        if m.complete { "" } else { " (cut short)" },
                        m.subject_name,
                        m.chars,
                        m.errors
                    ));
                    self.pending_events.push(navtex_event(&m, t, self.dial_hz));
                }
            }
            MarineReport::Fax(s) => {
                if s.state == 1 || s.state == 2 {
                    self.last_fax = t;
                }
                self.fax = Some((**s).clone());
            }
        }
    }

    /// The service heard last, the display state, and whether a decoder holds lock.
    fn active(&self) -> (Active, Hearing, bool) {
        let now = self.clock.now();
        let locked = |w: Locker| self.locks[w as usize];
        let fax_active = self
            .fax
            .as_ref()
            .is_some_and(|f| f.state == 1 || f.state == 2);
        let (nv, dm, _, _) = self.flags;
        let active = if locked(Locker::Navtex) {
            Active::Navtex
        } else if locked(Locker::Dsc) || locked(Locker::DscParallel) {
            Active::Dsc
        } else if locked(Locker::DscVhf) {
            Active::DscVhf
        } else if fax_active {
            Active::Fax
        } else {
            let mut best = (Active::None, f64::NEG_INFINITY);
            for (name, t) in [
                (Active::Navtex, self.last_navtex),
                (Active::Dsc, self.last_dsc),
                (Active::Fax, self.last_fax),
            ] {
                if now - t <= 120.0 && t > best.1 {
                    best = (name, t);
                }
            }
            if best.0 != Active::None {
                best.0
            } else if self.fsk.0.found && nv {
                Active::Navtex
            } else if self.fsk.0.found && dm {
                Active::Dsc
            } else {
                Active::None
            }
        };
        let any_lock = self.locks.iter().any(|l| *l);
        let state = if any_lock || fax_active {
            Hearing::Decoding
        } else if self.fsk.0.found
            || self.fax.as_ref().is_some_and(|f| f.state == 3)
            || active != Active::None
        {
            Hearing::Signal
        } else {
            Hearing::Searching
        };
        (active, state, any_lock)
    }

    /// Moves the scanner one report period on, and requests the host to move the radio when it needs another channel.
    fn scan_step(&mut self, any_lock: bool) -> Option<f64> {
        let s = self.scanner.as_mut()?;
        self.scan_clock += REPORT_S;
        s.tick(self.scan_clock, any_lock);
        let want = s.current().map(|khz| khz * 1000.0);
        if want != self.scan_asked {
            self.scan_asked = want;
            if let Some(hz) = want {
                self.pending_requests.push(HostRequest::Retune {
                    tuned_hz: hz,
                    reason: format!("DSC scan, {:.1} kHz", hz / 1000.0),
                });
            }
        }
        want
    }

    fn report(&mut self) {
        self.reports += 1;
        let now = self.clock.now();
        let (active, state, any_lock) = self.active();
        let scan_ch = self.scan_step(any_lock);
        let offset = if active == Active::Fax {
            self.fax.as_ref().map(|f| f.carrier_offset_hz)
        } else if self.fsk.0.found {
            Some(self.fsk.0.centre_hz)
        } else {
            None
        };
        let snr = if active == Active::Fax {
            self.fax.as_ref().and_then(|f| f.snr_db)
        } else if active == Active::DscVhf || !self.fsk.0.found {
            None
        } else {
            Some(self.fsk.0.snr_db)
        };
        let summary = if state == Hearing::Searching {
            "Marine: searching".to_string()
        } else if active == Active::Fax {
            let f = self.fax.clone().unwrap_or_default();
            format!(
                "Marine fax: {}, IOC {}, {} lpm, {} lines",
                f.state_name, f.ioc, f.lpm, f.lines
            )
        } else {
            let name = match active {
                Active::Dsc => "DSC",
                Active::DscVhf => "DSC VHF",
                _ => "NAVTEX",
            };
            format!(
                "Marine {name}: {}, {} NAVTEX, {} DSC",
                if state == Hearing::Decoding {
                    "decoding"
                } else {
                    "signal"
                },
                self.lists.navtex.messages.len(),
                self.lists.dsc.len()
            )
        };
        let (fsk, symbol_rate) = self.fsk;
        let l = &self.lists;
        let distress = l.dsc.iter().any(|e| e.call.distress);
        let p = &mut self.p;
        p.results_mut().set_status(&Status {
            lock_state: state.lock_state(),
            offset_hz: offset,
            snr_db: snr.map(|v| v as f32),
            summary,
            ..Status::default()
        });
        let found = |v: f64| fsk.found.then_some(v);
        let fax = self.fax.as_ref();
        p.put(&HfFields {
            state,
            active_service: active,
            fsk_found: fsk.found,
            fsk_low_hz: found(fsk.low_hz).map(Hertz),
            fsk_high_hz: found(fsk.high_hz).map(Hertz),
            shift_hz: found(fsk.shift_hz).map(Hertz),
            symbol_rate_hz: found(symbol_rate).map(Hertz),
            tone_level_db: found(fsk.level_db),
            snr_db: snr,
            offset_hz: offset.map(Hertz),
            navtex_good: l.navtex.good as i64,
            navtex_bad: l.navtex.bad as i64,
            dsc_good: l.dsc_good as i64,
            dsc_bad: l.dsc_bad as i64,
            distress,
            fax_state: fax.and_then(|f| FaxState::from_index(u16::from(f.state.min(3)))),
            ioc: fax.map(|f| f.ioc.into()),
            lpm: fax.map(|f| f.lpm.into()),
            lines: fax.map(|f| f.lines as i64),
            width: fax.map(|f| f.width as i64),
            slant_ppm: fax.map(|f| f.slant_ppm),
            black_level_hz: fax.map(|f| f.black_level_hz).map(Hertz),
            last_tone_hz: fax.map(|f| f.last_tone_hz).map(Hertz),
            phasing_lines: fax.map(|f| f.phasing_lines as i64),
            fax_snr_db: fax.and_then(|f| f.snr_db),
            pictures_started: fax.map(|f| f.pictures_started as i64),
            pictures_done: fax.map(|f| f.pictures_done as i64),
            end_reason: fax.map(|f| f.end_reason.as_str()),
            scan_channel_hz: scan_ch.map(Hertz),
            reports: self.reports as i64,
            signal_time: Seconds(now),
        });
        shaped::log_rows(&l.navtex.messages, &NAVTEX_TABLE, now, p);
        shaped::log_rows(&l.dsc, &DSC_TABLE, now, p);
        let r = p.results_mut();
        if let Some(id) = r.decl().series_id(SPECTRUM_SERIES) {
            let spectrum = &self.spectrum;
            r.update_series(id, |s| {
                let line = !s.line;
                s.line = true;
                copy_if_changed(&mut s.points, spectrum) || line
            });
        }
        // Each station placed by a checked call, at its newest position (the station in distress for relays and
        // acknowledgements).
        // Each point shows the row of the newest call that placed it.
        let table = r.decl().table_id(DSC_TABLE.def().key);
        let mut placed: Vec<String> = Vec::new();
        for (seq, e) in l.dsc.iter_seq() {
            let c = &e.call;
            let (Some(lat), Some(lon)) = (c.lat, c.lon) else {
                continue;
            };
            if !c.ecc_ok {
                continue;
            }
            let who = if c.distress_mmsi.is_empty() {
                &c.from
            } else {
                &c.distress_mmsi
            };
            let id = format!("{POINT_PREFIX}{who}");
            let label = format!("{who} {}", c.kind);
            r.update_point(&id, lat, lon, &label, None);
            let row = format!("{seq:06}");
            r.link_point(&id, table.map(|t| (t, row.as_str())));
            placed.push(id);
        }
        let gone: Vec<String> = r
            .points()
            .keys()
            .filter(|k| k.starts_with(POINT_PREFIX) && !placed.contains(k))
            .cloned()
            .collect();
        for k in &gone {
            r.remove_point(k);
        }
    }

    fn rebuild_scanner(&mut self) {
        self.scanner = if self.scan && !self.parallel {
            let list: Vec<(f64, u8)> = channels::scan_set(&self.scan_sets)
                .iter()
                .map(|c| (c.khz, c.weight))
                .collect();
            let mut sc = Scanner::new(&list, self.scan_dwell);
            sc.start(self.scan_clock);
            Some(sc)
        } else {
            None
        };
        self.scan_asked = None;
    }
}

/// The DSC scan.
pub const DSC_SCAN: Setting<field::Bool> =
    Setting::new("dsc_scan", "DSC scan", "", false).section("DSC");
/// The time the DSC scan stays on a channel.
pub const DSC_SCAN_DWELL_S: Setting<field::Duration> =
    Setting::new("dsc_scan_dwell_s", "DSC dwell", "s", 4.0)
        .range(1.0, 30.0, 0.0)
        .section("DSC");
/// The wall-clock time of the first sample of a recording.
pub const TIME_BASE: Setting<field::Time> = Setting::new("time_base", "Time base", "", 0.0)
    .range(0.0, 1e11, 0.0)
    .help("UNIX time of the first sample of a recording. 0 when not known. Live samples use the computer clock.");

sdrkit_core::settings! {
    /// The settings of [`MarineDisplay`].
    pub enum DisplaySetting {
        /// Which decoders run.
        Service = SERVICE,
        /// The DSC scan.
        Scan = DSC_SCAN,
        /// The DSC channel sets.
        ScanSets = DSC_SCAN_SETS,
        /// The DSC dwell.
        ScanDwell = DSC_SCAN_DWELL_S,
        /// Parallel DSC.
        Parallel = DSC_PARALLEL,
        /// The time base of a recording.
        TimeBase = TIME_BASE,
    }
}

sdrkit_core::commands! {
    /// The commands of [`MarineDisplay`].
    pub enum DisplayCommand {
        /// Forget every NAVTEX message and DSC call.
        Clear = objcmd::CLEAR,
    }
}

impl Component for MarineDisplay {
    fn info(&self) -> ComponentInfo {
        let ins = ["fsk", "dsc", "navtex", "vhf", "fax", "parallel"]
            .iter()
            .map(|n| Port::typed::<MarineReport>(n))
            .collect();
        ComponentInfo::processor(
            "marine.display",
            ins,
            vec![
                Port::new("events", Kind::Messages).rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<HostRequest>("requests").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
        .with_settings(DisplaySetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.dial_hz = inputs.first().map_or(0.0, dial_of);
        self.flags = services(self.service, self.dial_hz);
        self.locks = [false; 4];
        self.fax = None;
        // A new input: its item times start again at 0, the clock runs on.
        self.clock.restart(self.live, self.host_clock.now());
        Ok(vec![
            Format::new(Kind::Messages, 0.0),
            Format::new(Kind::typed::<HostRequest>(), 0.0),
        ])
    }
    fn attach(&mut self, ctx: &RunContext) {
        self.live = ctx.wall_clock();
        self.host_clock = ctx.clock.clone();
    }
    fn process(&mut self, io: &mut Io) {
        let mut items = std::mem::take(&mut self.items);
        items.clear();
        for i in 0..6 {
            items.extend(io.items::<MarineReport>(i).iter().cloned());
        }
        items.sort_by(|a, b| a.time.total_cmp(&b.time));
        for it in &items {
            let t = if matches!(it.value, MarineReport::Lock { .. }) {
                self.clock.now()
            } else {
                self.clock.at(it.time)
            };
            self.on_report(t, &it.value);
        }
        items.clear();
        self.items = items;
        if io.tick() {
            self.report();
        }
        io.messages_out(0).append(&mut self.pending_events);
        let now = self.clock.now();
        for r in self.pending_requests.drain(..) {
            io.items_out::<HostRequest>(1).push(Timed {
                time: now,
                value: r,
            });
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match DisplaySetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            DisplaySetting::Service => {
                self.service = SERVICE.read(id, v)?;
                self.flags = services(self.service, self.dial_hz);
            }
            DisplaySetting::Scan => {
                self.scan = DSC_SCAN.read(id, v)?;
                self.rebuild_scanner();
            }
            DisplaySetting::Parallel => {
                self.parallel = DSC_PARALLEL.read(id, v)?;
                self.rebuild_scanner();
            }
            DisplaySetting::ScanSets => {
                let t = DSC_SCAN_SETS.read(id, v)?;
                self.scan_sets = scan_sets(id, t)?;
                t.clone_into(&mut self.scan_sets_text);
                self.rebuild_scanner();
            }
            DisplaySetting::ScanDwell => {
                self.scan_dwell = DSC_SCAN_DWELL_S.read(id, v)?;
                self.rebuild_scanner();
            }
            DisplaySetting::TimeBase => {
                let t = TIME_BASE.read(id, v)?;
                self.time_base = t;
                self.clock.set_time_base((t > 0.0).then_some(t));
            }
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([
            (DisplaySetting::Service, Value::Choice(self.service.index())),
            (DisplaySetting::Scan, Value::Bool(self.scan)),
            (
                DisplaySetting::ScanSets,
                Value::Text(self.scan_sets_text.clone()),
            ),
            (DisplaySetting::ScanDwell, Value::Duration(self.scan_dwell)),
            (DisplaySetting::Parallel, Value::Bool(self.parallel)),
            (DisplaySetting::TimeBase, Value::Time(self.time_base)),
        ])
    }
    fn commands(&self) -> &Commands {
        DisplayCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, _: &Values) -> Result<Option<Record>, CommandError> {
        match DisplayCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            DisplayCommand::Clear => self.lists.clear(),
        }
        self.report();
        Ok(None)
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
    fn save_state(&self) -> Option<StateBlob> {
        self.lists.save_state(self.clock.now()).ok()
    }
    fn restore_state(&mut self, state: &StateBlob) -> Result<(), StateError> {
        self.lists.restore_state(state, self.clock.now())?;
        let (n, d) = (self.lists.navtex.messages.len(), self.lists.dsc.len());
        self.p.results_mut().log(format!(
            "Marine: loaded {n} NAVTEX messages and {d} DSC calls"
        ));
        self.report();
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------

/// Adds the marine HF components to `r`.
pub fn register(r: &mut Registry) {
    r.add("marine.fsk", || Box::new(FskComponent::default()));
    r.add("marine.dsc_mf", || Box::new(DscMfComponent::default()));
    r.add("marine.navtex", || Box::new(NavtexComponent::default()));
    r.add("marine.vhf_dsc", || Box::new(VhfDscComponent::default()));
    r.add("marine.usb", || Box::new(UsbComponent::default()));
    r.add("marine.fax", || Box::new(FaxComponent::default()));
    r.add("marine.listen", || Box::new(ListenComponent::default()));
    r.add("marine.parallel", || Box::new(ParallelDsc::default()));
    r.add("marine.display", || Box::new(MarineDisplay::default()));
}

/// The marine HF graph.
///
/// # Errors
///
/// [`GraphError`] if a component refuses its constants.
pub fn builder() -> Result<GraphBuilder, GraphError> {
    Ok(GraphBuilder::new()
        .add(
            "frontend",
            FixedRateFrontEnd::new(0.0, 10e3, CHANNEL_RATE).map_err(|error| {
                GraphError::Setting {
                    key: "frontend.offset".into(),
                    error,
                }
            })?,
        )
        .add("fsk", FskComponent::default())
        .add("dsc_mf", DscMfComponent::default())
        .add("navtex", NavtexComponent::default())
        .add("vhf_dsc", VhfDscComponent::default())
        .add("usb", UsbComponent::default())
        .add("fax", FaxComponent::default())
        .add("listen", ListenComponent::default())
        .add("parallel", ParallelDsc::default())
        .add("display", MarineDisplay::default())
        .input("frontend.in")
        .input("parallel.in")
        .connect("frontend.out", "fsk.in")
        .connect("fsk.soft", "dsc_mf.soft")
        .connect("fsk.soft", "navtex.soft")
        .connect("frontend.out", "vhf_dsc.in")
        .connect("frontend.out", "usb.in")
        .connect("usb.out", "fax.in")
        .connect("usb.out", "listen.in")
        .connect("fsk.report", "display.fsk")
        .connect("dsc_mf.report", "display.dsc")
        .connect("navtex.report", "display.navtex")
        .connect("vhf_dsc.report", "display.vhf")
        .connect("fax.report", "display.fax")
        .connect("parallel.report", "display.parallel")
        .output("display.events")
        .output("display.requests")
        .output("listen.out")
        .status_from("display")
        .clock(REPORT_S)
        .reconfigure_on_retune())
}

/// The settings of the marine HF stack.
pub(crate) const OWN: &[&dyn DeclareSetting] = &[
    &SERVICE,
    &FAX_LPM,
    &FAX_IOC,
    &FAX_AUTO_SLANT,
    &FAX_SLANT_PPM,
    &FAX_MAX_LINES,
    &FAX_ROTATE,
    &FAX_ROW_FILTER,
    &LISTEN,
    &DSC_SCAN,
    &DSC_SCAN_SETS,
    &DSC_SCAN_DWELL_S,
    &DSC_PARALLEL,
    &NAMES,
    &TIME_BASE,
];

/// The settings of the marine HF stack, then its test signal's.
pub fn schema() -> Arc<Schema> {
    static CELL: OnceLock<Arc<Schema>> = OnceLock::new();
    CELL.get_or_init(|| Arc::new(crate::typed::shape("marine").schema()))
        .clone()
}

/// The marine HF stack graph. A submode fixes the service: it wins over the `service` setting, whose default is the automatic
/// choice. The fax picture leaves on the port `fax.picture` on the picture stream.
///
/// # Errors
///
/// [`GraphError`] if a setting of `cfg` is refused.
pub fn stack_graph(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    let mut values = cfg.settings.clone();
    if let Some(s) = cfg.submode.as_deref().and_then(Service::of_submode) {
        let id = values
            .schema()
            .id(SERVICE.key())
            .map_err(|error| GraphError::Setting {
                key: SERVICE.key().into(),
                error,
            })?;
        values
            .set(id, &Value::Choice(s.index()))
            .map_err(|error| GraphError::Setting {
                key: SERVICE.key().into(),
                error,
            })?;
    }
    let mut rx = StackGraph::of(
        crate::typed::shape("marine"),
        builder()?.catalog(&Catalog::of(cfg)?).build()?,
    )
    .min_rate(
        MIN_RATE_HZ,
        "Marine needs a sample rate of at least 12 ksps",
    )
    .route(
        SERVICE.key(),
        &[
            "dsc_mf.service",
            "navtex.service",
            "vhf_dsc.service",
            "fax.service",
            "display.service",
        ],
    )?
    .route(LISTEN.key(), &["listen.listen"])?
    .route(
        DSC_PARALLEL.key(),
        &["parallel.dsc_parallel", "display.dsc_parallel"],
    )?
    .route(
        DSC_SCAN_SETS.key(),
        &["parallel.dsc_scan_sets", "display.dsc_scan_sets"],
    )?
    .route(NAMES.key(), &["dsc_mf.names", "vhf_dsc.names"])?
    .command("clear", sdrkit_graph::CommandTarget::of("display", "clear"))
    .command(
        "clear_picture",
        sdrkit_graph::CommandTarget::of("fax", "clear_picture"),
    );
    for s in &FaxSetting::SETTINGS[1..] {
        let k = s.key();
        rx = rx.route(k, &[&format!("fax.{k}")])?;
    }
    for s in [
        &DSC_SCAN as &dyn DeclareSetting,
        &DSC_SCAN_DWELL_S,
        &TIME_BASE,
    ] {
        let k = s.key();
        rx = rx.route(k, &[&format!("display.{k}")])?;
    }
    rx.with_settings(&values).build()
}
