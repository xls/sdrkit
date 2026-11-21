//! `modem.tone_detect`: tones in real audio found by a Goertzel tone bank, reported as they start and stop.

use super::parse_list;
use sdrkit_algo_modem::tone::{ToneDetector, ToneDetectorConfig};
use sdrkit_core::publish::field;
use sdrkit_core::EventDef;
use sdrkit_core::Field;
use sdrkit_core::Record;
use sdrkit_core::{Choice, SettingId, SettingSet};
use sdrkit_core::{ConfigError, Setting, SettingError, Value, Values};
use sdrkit_graph::values_of;
use sdrkit_graph::{Applied, Component, ComponentInfo, Format, Io, Item, Kind, Port, Timed};
use sdrkit_util_ids::tones::plan;

/// A tone starting or stopping (the output of `modem.tone_detect`). Its time is the start of the first block that showed the
/// change.
#[derive(Clone, Debug, PartialEq)]
pub struct ToneEvent {
    /// The tone's index in the plan (or in `tones_hz`).
    pub tone: u32,
    /// The tone's frequency.
    pub hz: f64,
    /// The tone's label in a named plan (`sdrkit_util_ids::tones::plan`: the selcall digit, the SELCAL designator, the CTCSS or DTMF
    /// frequency as text); empty for a list.
    pub label: &'static str,
    /// Started (`true`) or stopped.
    pub on: bool,
    /// The tone's mean power in the block that decided the change, dB relative to a full-scale sine's.
    pub level_db: f32,
}

impl Item for ToneEvent {
    const KIND: &'static str = "tone.event";
    fn event_def() -> Option<&'static EventDef> {
        Some(&TONE_EVENT)
    }
    fn write_fields(&self, r: &mut Record) {
        r.set(&TONE, i64::from(self.tone));
        r.set(&HZ, self.hz);
        r.set(&LABEL, self.label);
        r.set(&ON, self.on);
        r.set(&LEVEL_DB, f64::from(self.level_db));
    }
}

const TONE: Field<field::I64> = Field::new("tone", "Tone", "");
const HZ: Field<field::Hz> = Field::new("hz", "Tone frequency", "Hz");
const LABEL: Field<field::Text> = Field::new("label", "Label", "");
const ON: Field<field::Bool> = Field::new("on", "Started", "");
const LEVEL_DB: Field<field::F64> = Field::new("level_db", "Level", "dB");

static TONE_EVENT: EventDef =
    EventDef::new("tone.event", "Tone", &[&TONE, &HZ, &LABEL, &ON, &LEVEL_DB]);

sdrkit_core::choice_enum! {
    /// The tones [`ToneDetect`] listens for: a list, or a named plan of `sdrkit_util_ids::tones::plan`.
    pub enum TonePlan {
        /// The tones of `tones_hz`.
        List = ("list", "A list of tones"),
        /// CTCSS.
        Ctcss = ("ctcss", "CTCSS"),
        /// CTCSS (EIA).
        CtcssEia = ("ctcss_eia", "CTCSS (EIA)"),
        /// DTMF.
        Dtmf = ("dtmf", "DTMF"),
        /// SELCAL.
        Selcal = ("selcal", "SELCAL"),
        /// SELCAL (classic).
        SelcalClassic = ("selcal_classic", "SELCAL (classic)"),
        /// Selcall ZVEI1.
        SelcallZvei1 = ("selcall_zvei1", "Selcall ZVEI1"),
        /// Selcall ZVEI2.
        SelcallZvei2 = ("selcall_zvei2", "Selcall ZVEI2"),
        /// Selcall ZVEI3.
        SelcallZvei3 = ("selcall_zvei3", "Selcall ZVEI3"),
        /// Selcall PZVEI.
        SelcallPzvei = ("selcall_pzvei", "Selcall PZVEI"),
        /// Selcall DZVEI.
        SelcallDzvei = ("selcall_dzvei", "Selcall DZVEI"),
        /// Selcall PDZVEI.
        SelcallPdzvei = ("selcall_pdzvei", "Selcall PDZVEI"),
        /// Selcall CCIR1.
        SelcallCcir1 = ("selcall_ccir1", "Selcall CCIR1"),
        /// Selcall CCIR2.
        SelcallCcir2 = ("selcall_ccir2", "Selcall CCIR2"),
        /// Selcall PCCIR.
        SelcallPccir = ("selcall_pccir", "Selcall PCCIR"),
        /// Selcall EEA.
        SelcallEea = ("selcall_eea", "Selcall EEA"),
        /// Selcall EIA.
        SelcallEia = ("selcall_eia", "Selcall EIA"),
        /// Selcall EURO.
        SelcallEuro = ("selcall_euro", "Selcall EURO"),
        /// Selcall NATEL.
        SelcallNatel = ("selcall_natel", "Selcall NATEL"),
        /// Selcall MODAT.
        SelcallModat = ("selcall_modat", "Selcall MODAT"),
    }
}

impl TonePlan {
    /// The named plan, or `None` for a list.
    fn plan(self) -> Option<plan::Plan> {
        match self {
            TonePlan::List => None,
            p => plan::by_name(&Self::CHOICES[usize::from(p.index())].key),
        }
    }
}

const PLAN: Setting<field::Choice<TonePlan>> = Setting::choice("plan", "Plan", TonePlan::List).help("The tones to listen for: a list (tones_hz) or a named plan (CTCSS, DTMF, SELCAL, selcall sets).");
const TONES_HZ: Setting<field::Text> = Setting::new("tones_hz", "Tones", "", "")
    .help("The tone frequencies in Hz, separated by commas (plan list).");
const BLOCK_S: Setting<field::F64> = Setting::new("block_s", "Block", "s", 0.02)
    .range(0.001, 10.0, 0.0)
    .help(
        "The time each decision looks at: longer separates closer tones (about 2 / block apart).",
    );
const THRESHOLD_DB: Setting<field::F64> = Setting::new("threshold_db", "Threshold", "dB", -6.0)
    .range(-40.0, 0.0, 0.0)
    .help("A tone's power relative to the whole block's for it to count as present.");
const MIN_BLOCKS: Setting<field::I64> = Setting::new("min_blocks", "Blocks to decide", "", 2)
    .range(1.0, 100.0, 1.0)
    .help("Blocks in a row a tone must be present (or absent) before it starts (or stops).");

sdrkit_core::settings! {
    /// The settings of [`ToneDetect`].
    pub enum ToneDetectSetting {
        /// Plan.
        Plan = PLAN,
        /// Tones.
        TonesHz = TONES_HZ,
        /// Block.
        BlockS = BLOCK_S,
        /// Threshold.
        ThresholdDb = THRESHOLD_DB,
        /// Blocks to decide.
        MinBlocks = MIN_BLOCKS,
    }
}

/// `modem.tone_detect`: tones in `Real` audio, as [`ToneEvent`]s when each starts and stops.
///
/// A thin wrapper over `sdrkit_algo_modem::tone::ToneDetector`: every block of `block_s` seconds goes through one Goertzel filter per
/// tone (one pass for all of them); a tone is present in a block when its power is at least `threshold_db` relative to the
/// block's whole power; a tone starts or stops after `min_blocks` blocks in a row say so.
///
/// `plan` picks the tones: `list` takes them from `tones_hz`; a named plan takes them from the tables of `sdrkit-util-ids`
/// (`sdrkit_util_ids::tones::plan`: `ctcss`, `ctcss_eia`, `dtmf`, `selcal`, `selcal_classic`, `selcall_zvei1` and the other selcall
/// sets), in the plan's order, with each tone's label in the events. `block_s` must suit the plan: about `2 / block_s` Hz between
/// tones (10 ms for DTMF, 20 to 40 ms for selcall, 0.25 s for SELCAL, 0.8 s for all 50 CTCSS tones).
///
/// DCS is not a tone plan (it is a coded bit stream at 134.4 bit/s) and is not decoded here.
#[derive(Debug)]
pub struct ToneDetect {
    plan: TonePlan,
    tones_hz: String,
    block_s: f64,
    threshold_db: f64,
    min_blocks: u32,
    freqs: Vec<f64>,
    labels: Vec<&'static str>,
    det: Option<Box<ToneDetector>>,
    /// The absolute index of the detector's position 0.
    origin: u64,
    started: bool,
    rate: f64,
}

impl Default for ToneDetect {
    fn default() -> Self {
        ToneDetect {
            plan: TonePlan::List,
            tones_hz: String::new(),
            block_s: BLOCK_S.default,
            threshold_db: THRESHOLD_DB.default,
            min_blocks: MIN_BLOCKS.default as _,
            freqs: Vec::new(),
            labels: Vec::new(),
            det: None,
            origin: 0,
            started: false,
            rate: 0.0,
        }
    }
}

impl ToneDetect {
    /// The tones in Hz and their labels.
    fn tones(&self) -> Result<(Vec<f64>, Vec<&'static str>), ConfigError> {
        let Some(p) = self.plan.plan() else {
            let f = parse_list(&self.tones_hz).filter(|f| !f.is_empty()).ok_or(
                SettingError::Required {
                    id: ToneDetectSetting::TonesHz.id(),
                },
            )?;
            let n = f.len();
            return Ok((f, vec![""; n]));
        };
        Ok(p.iter().map(|(f, l)| (f as f64, l)).unzip())
    }
}

impl Component for ToneDetect {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "modem.tone_detect",
            vec![Port::real("in")],
            vec![Port::typed::<ToneEvent>("out")],
        )
        .with_settings(ToneDetectSetting::schema())
    }

    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        let rate = inputs[0].rate;
        let (freqs, labels) = self.tones()?;
        if freqs.iter().any(|&f| !(f > 0.0 && f < rate / 2.0)) {
            return Err(ConfigError::SettingAtRate {
                id: ToneDetectSetting::TonesHz.id(),
                rate_hz: rate,
            });
        }
        let cfg = ToneDetectorConfig {
            block: (self.block_s * rate).round() as usize,
            threshold_db: self.threshold_db as f32,
            min_blocks: self.min_blocks,
        };
        self.det = Some(Box::new(
            ToneDetector::with_rate(rate, &freqs, cfg).map_err(ConfigError::from)?,
        ));
        self.freqs = freqs;
        self.labels = labels;
        self.started = false;
        self.rate = rate;
        Ok(vec![Format::new(Kind::typed::<ToneEvent>(), 0.0)])
    }

    fn process(&mut self, io: &mut Io) {
        let x = io.real(0);
        let first = io.input(0).first();
        let Some(det) = self.det.as_mut() else { return };
        if !self.started {
            self.started = true;
            self.origin = first - det.position();
        }
        let (freqs, labels, origin, rate) = (&self.freqs, &self.labels, self.origin, self.rate);
        let out = io.items_out::<ToneEvent>(0);
        det.process(x, |c| {
            out.push(Timed {
                time: (origin + c.at) as f64 / rate,
                value: ToneEvent {
                    tone: c.tone as u32,
                    hz: freqs[c.tone],
                    label: labels[c.tone],
                    on: c.on,
                    level_db: c.level_db,
                },
            })
        });
    }

    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match ToneDetectSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            ToneDetectSetting::Plan => self.plan = PLAN.read(id, v)?,
            ToneDetectSetting::TonesHz => {
                let t = TONES_HZ.read(id, v)?;
                if parse_list(t).is_none() {
                    return Err(SettingError::Malformed { id, at: None });
                }
                self.tones_hz = t.to_owned();
            }
            ToneDetectSetting::BlockS => self.block_s = BLOCK_S.read(id, v)?,
            ToneDetectSetting::ThresholdDb => self.threshold_db = THRESHOLD_DB.read(id, v)?,
            ToneDetectSetting::MinBlocks => self.min_blocks = MIN_BLOCKS.read(id, v)? as u32,
        }
        Ok(Applied::Reformat)
    }

    fn values(&self) -> Values {
        values_of([
            (ToneDetectSetting::Plan, Value::Choice(self.plan.index())),
            (
                ToneDetectSetting::TonesHz,
                Value::Text(self.tones_hz.clone()),
            ),
            (ToneDetectSetting::BlockS, Value::F64(self.block_s)),
            (
                ToneDetectSetting::ThresholdDb,
                Value::F64(self.threshold_db),
            ),
            (
                ToneDetectSetting::MinBlocks,
                Value::I64(self.min_blocks as i64),
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_named_plans_are_the_plans_of_the_tables() {
        let keys: Vec<&str> = TonePlan::CHOICES[1..].iter().map(|c| &*c.key).collect();
        assert_eq!(keys, plan::NAMES);
        for i in 1..TonePlan::CHOICES.len() {
            assert!(TonePlan::from_index(i as u16).unwrap().plan().is_some());
        }
    }
}
