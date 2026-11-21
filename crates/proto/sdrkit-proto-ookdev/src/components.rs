//! The graph components `ookdev.pulses` and `ookdev.burst_pulses` (complex samples or bursts to pulse trains) and
//! `ookdev.decode` (pulse trains to device readings over a protocol list).

use crate::detect::{DetectorConfig, PulseDetector};
use crate::protocol::{Decoding, ProtocolList};
use crate::reading::Reading;
use crate::train::{Modulation, Train};
use sdrkit_components::modem::sync::IqBurst;
use sdrkit_core::interface::{Family, Fields, StackShape};
use sdrkit_core::publish::{field, keys, Published};
use sdrkit_core::{
    Choice, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    no_values, values_of, walk_tags, Applied, Component, ComponentInfo, Format, Io, Kind, Port,
    RateFrom, Registry, TagKind, TagStep, Timed,
};
use std::sync::{Arc, OnceLock};

/// The interface of the result sets of the components and of the reading event, which stacks of several families
/// use. A family whose stacks list one of them names this interface with `Family::uses`.
pub fn interface() -> &'static Family {
    static SHARED: OnceLock<Family> = OnceLock::new();
    SHARED.get_or_init(|| {
        Family::shared("ookdev", 1, 0).stack(
            StackShape::new("ookdev")
                .component("pulses", |c| c.fields::<PulsesFields>())
                .component("burst_pulses", |c| c.fields::<BurstPulsesFields>())
                .component("decode", |c| c.fields::<DecodeFields>())
                .event(crate::reading::reading()),
        )
    })
}

/// Adds `ookdev.pulses` and `ookdev.burst_pulses` to `r`.
pub fn register(r: &mut Registry) {
    r.add("ookdev.pulses", || Box::new(Pulses::default()));
    r.add("ookdev.burst_pulses", || Box::new(BurstPulses::default()));
}

/// Adds `ookdev.decode` with `list` to `r`.
pub fn register_decoder(r: &mut Registry, list: Arc<ProtocolList>) {
    r.add("ookdev.decode", move || Box::new(Decode::new(list.clone())));
}

sdrkit_core::choice_enum! {
    /// The modulations the pulse detector looks for.
    pub enum ModulationChoice {
        /// OOK and FSK.
        Auto = ("auto", "OOK and FSK"),
        /// OOK only.
        Ook = ("ook", "OOK"),
        /// FSK only.
        Fsk = ("fsk", "FSK"),
    }
}

impl ModulationChoice {
    /// The modulation of the detector configuration (`None` for both).
    fn modulation(self) -> Option<Modulation> {
        match self {
            ModulationChoice::Auto => None,
            ModulationChoice::Ook => Some(Modulation::Ook),
            ModulationChoice::Fsk => Some(Modulation::Fsk),
        }
    }
}

const MODULATION: Setting<field::Choice<ModulationChoice>> =
    Setting::choice("modulation", "Modulation", ModulationChoice::Auto);
const THRESHOLD_DB: Setting<field::F64> =
    Setting::new("threshold_db", "Open level", "dB", 10.0).range(3.0, 40.0, 0.0);
const CLOSE_DB: Setting<field::F64> = Setting::new("close_db", "Close level", "dB", 6.0)
    .range(0.0, 40.0, 0.0)
    .help("At most the open level.");
const RESET_US: Setting<field::F64> =
    Setting::new("reset_us", "End of a train", "µs", 10_000.0).range(100.0, 1e6, 0.0);
const MIN_DEVIATION_HZ: Setting<field::Hz> =
    Setting::new("min_deviation_hz", "Smallest FSK deviation", "Hz", 4000.0).range(0.0, 1e6, 0.0);
sdrkit_core::settings! {
    /// The settings of [`Pulses`].
    pub enum PulsesSetting {
        /// Modulation.
        Modulation = MODULATION,
        /// Open level.
        ThresholdDb = THRESHOLD_DB,
        /// Close level.
        CloseDb = CLOSE_DB,
        /// End of a train.
        ResetUs = RESET_US,
        /// Smallest FSK deviation.
        MinDeviationHz = MIN_DEVIATION_HZ,
    }
}

sdrkit_core::settings! {
    /// The settings of [`BurstPulses`].
    pub enum BurstPulsesSetting {
        /// Modulation.
        Modulation = MODULATION,
        /// Open level.
        ThresholdDb = THRESHOLD_DB,
        /// Close level.
        CloseDb = CLOSE_DB,
        /// End of a train.
        ResetUs = RESET_US,
        /// Smallest FSK deviation.
        MinDeviationHz = MIN_DEVIATION_HZ,
    }
}

/// The count of `ookdev.pulses`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct PulsesFields {
    /// Pulse trains made.
    #[field(label = "Trains")]
    pub trains: i64,
}

/// The counts of `ookdev.burst_pulses`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BurstPulsesFields {
    /// Pulse trains made.
    #[field(label = "Trains")]
    pub trains: i64,
    /// Bursts that made no train.
    #[field(shared = keys::DROPPED)]
    pub dropped: i64,
}

/// The counts of `ookdev.decode`, and of a decoder of a stack that offers trains to a protocol list.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct DecodeFields {
    /// Readings the protocols made.
    #[field(label = "Readings")]
    pub readings: i64,
    /// Trains no protocol decoded.
    #[field(label = "Unknown trains")]
    pub unknown_trains: i64,
}

/// A count as a whole number of a result field.
fn count(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

/// One detector setting, the same in both detector components.
#[derive(Clone, Copy, Debug)]
enum Knob {
    Modulation,
    ThresholdDb,
    CloseDb,
    ResetUs,
    MinDeviationHz,
}

/// The detector settings both detector components share.
#[derive(Clone, Copy, Debug)]
struct Knobs {
    modulation: ModulationChoice,
    threshold_db: f64,
    close_db: f64,
    reset_us: f64,
    min_deviation_hz: f64,
}

impl Default for Knobs {
    fn default() -> Self {
        Knobs {
            modulation: MODULATION.default,
            threshold_db: THRESHOLD_DB.default,
            close_db: CLOSE_DB.default,
            reset_us: RESET_US.default,
            min_deviation_hz: MIN_DEVIATION_HZ.default,
        }
    }
}

impl Knobs {
    fn apply(&mut self, knob: Knob, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match knob {
            Knob::Modulation => self.modulation = MODULATION.read(id, v)?,
            Knob::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            Knob::CloseDb => self.close_db = CLOSE_DB.read(id, v)?,
            Knob::ResetUs => self.reset_us = RESET_US.read(id, v)?,
            Knob::MinDeviationHz => self.min_deviation_hz = MIN_DEVIATION_HZ.read(id, v)?,
        }
        Ok(Applied::Reformat)
    }

    /// Makes sure that the close level is at most the open level. `close` and `threshold` are the ids of the two levels in the
    /// component's schema.
    fn check(&self, close: SettingId, threshold: SettingId) -> Result<(), ConfigError> {
        if self.close_db > self.threshold_db {
            return Err(SettingError::Conflict {
                id: close,
                with: threshold,
            }
            .into());
        }
        Ok(())
    }

    /// The pulse detector at `rate`. `close` and `threshold` are the ids of the two levels in the component's schema.
    fn detector(
        &self,
        rate: f64,
        close: SettingId,
        threshold: SettingId,
    ) -> Result<PulseDetector, ConfigError> {
        if !(rate > 0.0 && rate.is_finite()) {
            return Err(ConfigError::NoRate);
        }
        self.check(close, threshold)?;
        let mut c = DetectorConfig::new(rate);
        c.modulation = self.modulation.modulation();
        c.open_db = self.threshold_db as f32;
        c.close_db = self.close_db as f32;
        c.reset_us = self.reset_us as f32;
        c.min_deviation_hz = self.min_deviation_hz;
        Ok(PulseDetector::new(c)?)
    }
}

/// `ookdev.pulses`: complex samples in, one typed [`Train`] out per train .
#[derive(Default)]
pub struct Pulses {
    knobs: Knobs,
    det: Option<PulseDetector>,
    base: u64,
    trains: u64,
    results: Option<Published>,
}

impl Component for Pulses {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ookdev.pulses",
            vec![Port::iq("in")],
            vec![Port::typed::<Train>("trains").rate_from(RateFrom::Fixed(0.0))],
        )
        .manual_tags()
        .with_settings(PulsesSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.det = Some(self.knobs.detector(
            inputs[0].rate,
            PulsesSetting::CloseDb.id(),
            PulsesSetting::ThresholdDb.id(),
        )?);
        self.base = 0;
        self.trains = 0;
        let mut p = Published::new(PulsesFields::SHAPE.fields);
        p.put(&PulsesFields { trains: 0 });
        self.results = Some(p);
        Ok(vec![Format::new(Kind::typed::<Train>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let x = io.iq(0);
        let input = io.input(0);
        let first = input.first();
        let Some(det) = self.det.as_mut() else { return };
        let rate = det.config().rate_hz;
        let out = io.items_out::<Train>(0);
        let base = &mut self.base;
        let before = out.len();
        walk_tags(first, x.len(), input.tags(), |step| match step {
            TagStep::Items(a, b) => det.process(&x[a..b], |t| {
                let mut t = t.clone();
                t.start += *base as f64;
                out.push(Timed {
                    time: t.start / rate,
                    value: t,
                });
            }),
            TagStep::Tag(at, t) => {
                if matches!(t.kind, TagKind::Retuned { .. }) {
                    det.reset();
                    *base = first + at as u64;
                }
            }
        });
        let made = out.len() - before;
        if made > 0 {
            self.trains += made as u64;
            if let Some(r) = self.results.as_mut() {
                r.put(&PulsesFields {
                    trains: count(self.trains),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let knob = match PulsesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            PulsesSetting::Modulation => Knob::Modulation,
            PulsesSetting::ThresholdDb => Knob::ThresholdDb,
            PulsesSetting::CloseDb => Knob::CloseDb,
            PulsesSetting::ResetUs => Knob::ResetUs,
            PulsesSetting::MinDeviationHz => Knob::MinDeviationHz,
        };
        self.knobs.apply(knob, id, v)
    }
    fn values(&self) -> Values {
        let k = &self.knobs;
        values_of([
            (
                PulsesSetting::Modulation,
                Value::Choice(k.modulation.index()),
            ),
            (PulsesSetting::ThresholdDb, Value::F64(k.threshold_db)),
            (PulsesSetting::CloseDb, Value::F64(k.close_db)),
            (PulsesSetting::ResetUs, Value::F64(k.reset_us)),
            (PulsesSetting::MinDeviationHz, Value::Hz(k.min_deviation_hz)),
        ])
    }
    fn results(&self) -> Option<&Results> {
        self.results.as_ref().map(|r| r.results())
    }
}

/// `ookdev.burst_pulses`: the bursts of `modem.burst_detect` in, their trains out. The floor and the sample rate of each
/// burst come from the burst detector. A burst whose rate returns no detector is dropped and counted.
#[derive(Default)]
pub struct BurstPulses {
    knobs: Knobs,
    det: Option<PulseDetector>,
    trains: u64,
    dropped: u64,
    results: Option<Published>,
}

impl Component for BurstPulses {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ookdev.burst_pulses",
            vec![Port::typed::<IqBurst>("bursts")],
            vec![Port::typed::<Train>("trains").rate_from(RateFrom::Fixed(0.0))],
        )
        .with_settings(BurstPulsesSetting::schema())
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.knobs.check(
            BurstPulsesSetting::CloseDb.id(),
            BurstPulsesSetting::ThresholdDb.id(),
        )?;
        self.det = None;
        self.trains = 0;
        self.dropped = 0;
        let mut p = Published::new(BurstPulsesFields::SHAPE.fields);
        p.put(&BurstPulsesFields {
            trains: 0,
            dropped: 0,
        });
        self.results = Some(p);
        Ok(vec![Format::new(Kind::typed::<Train>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let bursts = io.items::<IqBurst>(0);
        if bursts.is_empty() {
            return;
        }
        let out = io.items_out::<Train>(0);
        let before = out.len();
        let dropped = self.dropped;
        for b in bursts {
            let rate = b.value.rate_hz;
            if self.det.as_ref().is_none_or(|d| d.config().rate_hz != rate) {
                self.det = self
                    .knobs
                    .detector(
                        rate,
                        BurstPulsesSetting::CloseDb.id(),
                        BurstPulsesSetting::ThresholdDb.id(),
                    )
                    .ok();
            }
            let Some(det) = self.det.as_mut() else {
                self.dropped += 1;
                continue;
            };
            let start = b.value.start;
            det.run_burst(&b.value.samples, b.value.floor, |t| {
                let mut t = t.clone();
                t.start += start as f64;
                out.push(Timed {
                    time: t.start / rate,
                    value: t,
                });
            });
        }
        let made = out.len() - before;
        self.trains += made as u64;
        if made > 0 || self.dropped != dropped {
            if let Some(r) = self.results.as_mut() {
                r.put(&BurstPulsesFields {
                    trains: count(self.trains),
                    dropped: count(self.dropped),
                });
            }
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        let knob = match BurstPulsesSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            BurstPulsesSetting::Modulation => Knob::Modulation,
            BurstPulsesSetting::ThresholdDb => Knob::ThresholdDb,
            BurstPulsesSetting::CloseDb => Knob::CloseDb,
            BurstPulsesSetting::ResetUs => Knob::ResetUs,
            BurstPulsesSetting::MinDeviationHz => Knob::MinDeviationHz,
        };
        self.knobs.apply(knob, id, v)
    }
    fn values(&self) -> Values {
        let k = &self.knobs;
        values_of([
            (
                BurstPulsesSetting::Modulation,
                Value::Choice(k.modulation.index()),
            ),
            (BurstPulsesSetting::ThresholdDb, Value::F64(k.threshold_db)),
            (BurstPulsesSetting::CloseDb, Value::F64(k.close_db)),
            (BurstPulsesSetting::ResetUs, Value::F64(k.reset_us)),
            (
                BurstPulsesSetting::MinDeviationHz,
                Value::Hz(k.min_deviation_hz),
            ),
        ])
    }
    fn results(&self) -> Option<&Results> {
        self.results.as_ref().map(|r| r.results())
    }
}

/// `ookdev.decode`: trains in. Readings out, and on the second output the trains with at least 8 pulses that no protocol
/// decoded. A stack adds it with its own protocol list ([`register_decoder`]).
pub struct Decode {
    list: Arc<ProtocolList>,
    work: Decoding,
    found: Vec<Reading>,
    readings: u64,
    unknown: u64,
    results: Published,
}

impl Decode {
    /// A decoder over `list`.
    pub fn new(list: Arc<ProtocolList>) -> Self {
        Decode {
            list,
            work: Decoding::new(),
            found: Vec::new(),
            readings: 0,
            unknown: 0,
            results: Published::new(DecodeFields::SHAPE.fields),
        }
    }
}

/// Trains with fewer pulses than this are not output as unknown trains.
const UNKNOWN_MIN_PULSES: usize = 8;

impl Decode {
    fn put(&mut self) {
        self.results.put(&DecodeFields {
            readings: count(self.readings),
            unknown_trains: count(self.unknown),
        });
    }
}

impl Component for Decode {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "ookdev.decode",
            vec![Port::typed::<Train>("trains")],
            vec![
                Port::typed::<Reading>("readings").rate_from(RateFrom::Fixed(0.0)),
                Port::typed::<Train>("unknown").rate_from(RateFrom::Fixed(0.0)),
            ],
        )
    }
    fn configure(&mut self, _inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.put();
        Ok(vec![
            Format::new(Kind::typed::<Reading>(), 0.0),
            Format::new(Kind::typed::<Train>(), 0.0),
        ])
    }
    fn process(&mut self, io: &mut Io) {
        let trains = io.items::<Train>(0);
        if trains.is_empty() {
            return;
        }
        let (mut good, mut unknown) = (0u64, 0u64);
        for t in trains {
            self.found.clear();
            let n = self
                .work
                .run(&self.list, &t.value, &mut self.found)
                .unwrap_or(0);
            if n == 0 {
                if t.value.pulses.len() >= UNKNOWN_MIN_PULSES {
                    io.items_out::<Train>(1).push(t.clone());
                    unknown += 1;
                }
            } else {
                good += self.found.len() as u64;
                io.items_out::<Reading>(0)
                    .extend(self.found.drain(..).map(|r| Timed {
                        time: t.time,
                        value: r,
                    }));
            }
        }
        self.readings += good;
        self.unknown += unknown;
        self.put();
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
}
