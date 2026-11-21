//! The file format of band plans: band groups, countries and ITU regions, as JSON. The plan files a host loads and the files a
//! user imports use this one format, this one reader ([`parse`]) and these checks ([`check`]). `docs/schema/plans.json` is the
//! JSON Schema of the format.

use sdrkit_core::Choice;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The format version this library reads and writes.
pub const FORMAT_VERSION: u32 = 1;
/// The most channels one band group can expand to.
pub const MAX_CHANNELS: usize = 10_000;
/// The most tuning points one band can have.
pub const MAX_RASTER_POINTS: usize = 200_000;
/// The highest frequency a plan can name, Hz.
pub const MAX_HZ: f64 = 300e9;

sdrkit_core::choice_enum! {
    /// What a band group is for. A host shows the groups of one kind in one picker.
    #[derive(PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum PlanKind {
        /// FM broadcast (the dial range and raster).
        Fm = ("fm", "FM broadcast"),
        /// DAB, DAB+ and T-DMB ensembles.
        Dab = ("dab", "DAB"),
        /// Digital terrestrial TV (DVB-T, DVB-T2, ATSC, ISDB-T, DTMB).
        Tv = ("tv", "Digital TV"),
        /// Analogue terrestrial TV, with the vision and sound carriers.
        AnalogTv = ("analog_tv", "Analogue TV"),
        /// Cable TV (DVB-C and the analogue cable rasters).
        Cable = ("cable", "Cable TV"),
        /// Amateur bands of one IARU region.
        Amateur = ("amateur", "Amateur"),
        /// Long, medium and short wave broadcasting of one ITU region.
        Broadcast = ("broadcast", "Broadcast"),
        /// Marine VHF channels.
        Marine = ("marine", "Marine"),
        /// Aeronautical channels and bands.
        Aviation = ("aviation", "Aviation"),
        /// Licence-free personal radio (PMR446, FRS, GMRS, MURS, CB, UHF CB, LPD).
        Personal = ("personal", "Personal radio"),
        /// ISM and short-range device bands.
        Ism = ("ism", "ISM"),
        /// Anything else (user plans).
        Other = ("other", "Other"),
    }
}

impl PlanKind {
    /// Every kind, in picker order.
    pub const ALL: [PlanKind; 12] = [
        PlanKind::Fm,
        PlanKind::Dab,
        PlanKind::Tv,
        PlanKind::AnalogTv,
        PlanKind::Cable,
        PlanKind::Amateur,
        PlanKind::Broadcast,
        PlanKind::Marine,
        PlanKind::Aviation,
        PlanKind::Personal,
        PlanKind::Ism,
        PlanKind::Other,
    ];

    /// The kind with this id (any case), or `None`.
    pub fn from_id(id: &str) -> Option<PlanKind> {
        Self::ALL
            .into_iter()
            .find(|k| k.key().eq_ignore_ascii_case(id.trim()))
    }
}

/// The parameters of an analogue TV system.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalogSystem {
    /// The system letters ("B/G", "I", "D/K", "L", "M", "N").
    pub system: String,
    /// The colour systems used with it ("PAL", "SECAM", "NTSC").
    pub colour: String,
    /// The vision carrier above the lower channel edge, Hz.
    pub vision_above_lower_hz: f64,
    /// The main sound carrier above the vision carrier, Hz.
    pub sound_spacing_hz: f64,
    /// The sound is amplitude modulated (system L).
    #[serde(default, skip_serializing_if = "is_false")]
    pub am_sound: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_true(b: &bool) -> bool {
    *b
}

fn one() -> u32 {
    1
}

fn is_one(v: &u32) -> bool {
    *v == 1
}

fn yes() -> bool {
    true
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// One plan file: band groups, country entries and ITU region entries. Each part is optional.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanFile {
    /// The schema the file follows (a path or URL), for editors. The reader ignores it.
    #[serde(rename = "$schema", default, skip_serializing_if = "String::is_empty")]
    pub schema: String,
    /// The format version, [`FORMAT_VERSION`].
    pub format: u32,
    /// Band groups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<BandGroup>,
    /// Countries: the groups each uses and its presets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub countries: Vec<CountryEntry>,
    /// ITU regions: the groups and presets of a country that has none of its own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<RegionEntry>,
}

/// A published document that data comes from.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Who publishes it ("ACMA", "ITU", "FCC", "Ofcom", "BNetzA").
    pub publisher: String,
    /// Its title.
    pub title: String,
    /// Its number, edition or legal id ("47 CFR 73.603", "F2025L01088", "ECC/DEC/(15)05").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub number: String,
    /// Its date or version ("2026-07", "2021-11-22").
    pub date: String,
    /// The clause, table or page the values come from.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub clause: String,
    /// Where to read it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub url: String,
}

impl Source {
    /// The source as one line of text: publisher, title, number, date and clause.
    pub fn cite(&self) -> String {
        [
            self.publisher.as_str(),
            self.title.as_str(),
            self.number.as_str(),
            self.date.as_str(),
            self.clause.as_str(),
        ]
        .iter()
        .filter(|s| !s.trim().is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
    }
}

/// A band group: a named set of bands from one published plan, such as the Australian TV channel plan. Countries and ITU
/// regions name the groups they use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandGroup {
    /// Stable id ("tv_au", "fm_europe", "ham_r1"): 1 to 64 lower-case letters, digits, '_' or '-'.
    pub id: String,
    /// The name shown.
    pub name: String,
    /// What it is for.
    pub kind: PlanKind,
    /// The published document its values come from.
    pub source: Source,
    /// More documents that give some of its values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<Source>,
    /// Every value has been compared with the source.
    pub verified: bool,
    /// Why it is not verified (required when `verified` is false).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unverified_reason: String,
    /// A note for the picker.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The date the service of this group ended ("2013", "2013-12" or "2013-12-10"). Empty while the service is live.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ended: String,
    /// The analogue TV system, for analogue TV groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analog: Option<AnalogSystem>,
    /// Groups whose bands come first, before this group's own bands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
    /// The bands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bands: Vec<Band>,
}

/// The radio service of a band, after the service names of the ITU Radio Regulations Article 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Service {
    /// Sound or television broadcasting.
    Broadcasting,
    /// Mobile service (for example cellular networks).
    Mobile,
    /// Land mobile service (including licence-free personal radio).
    LandMobile,
    /// Maritime mobile service.
    MaritimeMobile,
    /// Aeronautical mobile service.
    AeronauticalMobile,
    /// Aeronautical radionavigation service.
    AeronauticalRadionavigation,
    /// Amateur service.
    Amateur,
    /// Radio astronomy service.
    RadioAstronomy,
    /// Industrial, scientific and medical applications.
    Ism,
    /// Short-range devices.
    ShortRange,
    /// Cable distribution (not a radio service).
    Cable,
}

impl Service {
    /// The id used in files ("broadcasting", "land_mobile").
    pub fn id(self) -> &'static str {
        match self {
            Service::Broadcasting => "broadcasting",
            Service::Mobile => "mobile",
            Service::LandMobile => "land_mobile",
            Service::MaritimeMobile => "maritime_mobile",
            Service::AeronauticalMobile => "aeronautical_mobile",
            Service::AeronauticalRadionavigation => "aeronautical_radionavigation",
            Service::Amateur => "amateur",
            Service::RadioAstronomy => "radio_astronomy",
            Service::Ism => "ism",
            Service::ShortRange => "short_range",
            Service::Cable => "cable",
        }
    }

    /// Every service, in file order.
    pub const ALL: [Service; 11] = [
        Service::Broadcasting,
        Service::Mobile,
        Service::LandMobile,
        Service::MaritimeMobile,
        Service::AeronauticalMobile,
        Service::AeronauticalRadionavigation,
        Service::Amateur,
        Service::RadioAstronomy,
        Service::Ism,
        Service::ShortRange,
        Service::Cable,
    ];
}

/// One band of a group: its edges, the stacks that receive it, and its channels or tuning raster.
///
/// A band holds numbered channels (`numbering`), listed channels (`channels`), both, or neither. A band without channels can
/// have a tuning raster (`raster`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Band {
    /// Its name ("VHF Band III", "UHF", "40 m", "MW").
    pub name: String,
    /// The radio service.
    pub service: Service,
    /// The lower edge, Hz.
    pub start_hz: f64,
    /// The upper edge, Hz.
    pub end_hz: f64,
    /// The ids of the stacks that receive it, in the order to try (`["dvbt", "dvbt2"]`). Empty when no stack does.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modes: Vec<String>,
    /// The band is used for the group's service. False when it moved to another service (a reallocated TV band).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub in_use: bool,
    /// The usual modulation there, as a modulation id ("lsb", "usb", "am", "nfm", "wfm").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub modulation: String,
    /// A note (required when `in_use` is false: what the band is used for now).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The width of each channel, Hz, unless a listed channel sets its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_width_hz: Option<f64>,
    /// Numbered channels on a regular raster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numbering: Option<Numbering>,
    /// Channels listed one by one.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<BandChannel>,
    /// The tuning raster of a band without channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raster: Option<Raster>,
    /// The channel numbers of a cellular band (3GPP): ARFCN, EARFCN, NR-ARFCN and GSCN.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cellular: Option<Cellular>,
}

/// A cellular system with numbered channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellSystem {
    /// GSM: ARFCN (3GPP TS 45.005).
    Gsm,
    /// LTE (E-UTRA): EARFCN (3GPP TS 36.101).
    Lte,
    /// 5G NR: NR-ARFCN and GSCN (3GPP TS 38.104).
    Nr,
}

/// How a cellular band uses its frequencies.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Duplex {
    /// Frequency division duplex: a downlink band and an uplink band.
    Fdd,
    /// Time division duplex: one band for both directions.
    Tdd,
    /// Supplementary downlink: a downlink band only.
    Sdl,
}

/// The channel numbering of a cellular band. The band edges (`start_hz`, `end_hz`) are the downlink edges.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cellular {
    /// The system.
    pub system: CellSystem,
    /// The band number (LTE band 20 is 20, NR band n78 is 78). GSM uses the band name (900, 850, 1800, 1900).
    pub band: u16,
    /// How the band uses its frequencies.
    pub duplex: Duplex,
    /// The downlink channel numbers, in one or more ranges.
    pub numbers: Vec<CellNumbers>,
    /// The NR synchronisation raster (GSCN), one range for each SSB subcarrier spacing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sync: Vec<SyncRaster>,
}

/// A range of downlink channel numbers: number `n` (from `first` to `last`, in steps of `step`) is at
/// `first_hz + (n - first) * hz_per_number`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellNumbers {
    /// The first number.
    pub first: u32,
    /// The last number.
    pub last: u32,
    /// The numbers between two usable channels (NR: 20 for the 100 kHz raster).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub step: u32,
    /// The frequency of number `first`, Hz.
    pub first_hz: f64,
    /// The frequency difference between two numbers, Hz.
    pub hz_per_number: f64,
    /// The subcarrier spacing this range is for, kHz (NR bands with a 15 or 30 kHz channel raster).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scs_khz: Option<u32>,
}

/// The GSCN range of an NR band for one SSB subcarrier spacing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncRaster {
    /// The SSB subcarrier spacing, kHz.
    pub scs_khz: u32,
    /// The first GSCN.
    pub first: u32,
    /// The last GSCN.
    pub last: u32,
    /// The GSCN step.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub step: u32,
}

impl CellNumbers {
    /// The frequency of number `n`, Hz, or `None` when `n` is not in the range.
    pub fn hz(&self, n: u32) -> Option<f64> {
        let k = n.checked_sub(self.first)?;
        (n <= self.last && k.is_multiple_of(self.step.max(1)))
            .then(|| self.first_hz + f64::from(k) * self.hz_per_number)
    }

    /// The number nearest to `hz` in the range, and its distance from `hz` in Hz. `None` when `hz` rounds outside the range.
    pub fn nearest(&self, hz: f64) -> Option<(u32, f64)> {
        let step = f64::from(self.step.max(1)) * self.hz_per_number;
        let k = ((hz - self.first_hz) / step).round();
        if !(k >= 0.0 && k <= f64::from(self.last.saturating_sub(self.first) / self.step.max(1))) {
            return None;
        }
        let n = self.first + k as u32 * self.step.max(1);
        Some((n, (self.hz(n)? - hz).abs()))
    }
}

/// Numbered channels: channel `n` (from `first` to `last`) is centred on `first_centre_hz + (n - first) * step_hz + offset_hz`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Numbering {
    /// The first channel number.
    pub first: u32,
    /// The last channel number.
    pub last: u32,
    /// The centre of the first channel, without `offset_hz`, Hz.
    pub first_centre_hz: f64,
    /// The spacing of the channels, Hz.
    pub step_hz: f64,
    /// A fixed offset added to every centre, Hz (ISDB-T: one seventh of a MHz).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset_hz: f64,
    /// Text put before each number ("E", "R", "DS", "PMR ").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub prefix: String,
    /// How each channel is named.
    #[serde(default, skip_serializing_if = "Naming::is_number")]
    pub names: Naming,
}

/// How numbered channels are named.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Naming {
    /// The prefix and the number ("E5", "21", "PMR 3").
    #[default]
    Number,
    /// The ICAO name of an 8.33 kHz channel ("118.005"), from its frequency.
    #[serde(rename = "icao_8_33")]
    Icao833,
}

impl Naming {
    fn is_number(&self) -> bool {
        *self == Naming::Number
    }
}

/// One listed channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BandChannel {
    /// Its name as the plan prints it ("9A", "16", "UHF 5").
    pub name: String,
    /// Its centre frequency, Hz.
    pub centre_hz: f64,
    /// Its width, Hz (default: the band's `channel_width_hz`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width_hz: Option<f64>,
    /// The other frequency of a duplex pair (a coast station, a repeater), Hz.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair_hz: Option<f64>,
    /// A short note ("distress and calling").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The channel is used for the band's service. False for a channel kept out of use (a reserved channel).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub in_use: bool,
}

/// The tuning raster of a band without channels: points at `first_hz + k * step_hz` up to the upper edge.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raster {
    /// The first point, Hz.
    pub first_hz: f64,
    /// The spacing, Hz.
    pub step_hz: f64,
}

/// How FM receivers read RDS: RDS (IEC 62106) or RBDS (NRSC-4-B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RdsVariant {
    /// RDS (IEC 62106).
    Rds,
    /// RBDS (NRSC-4-B): its own programme-type table and call letters from the PI code.
    Rbds,
}

/// The region presets that no band group holds.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presets {
    /// The FM pre-emphasis time constant, microseconds (ITU-R BS.450: 50 or 75).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fm_deemphasis_us: Option<f64>,
    /// How FM receivers read RDS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rds: Option<RdsVariant>,
    /// The stack id of the digital TV standard to try first ("dvbt", "dvbt2", "atsc", "isdbt", "dtmb").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tv_standard: Option<String>,
}

impl Presets {
    /// True when no preset is set.
    pub fn is_empty(&self) -> bool {
        self.fm_deemphasis_us.is_none() && self.rds.is_none() && self.tv_standard.is_none()
    }

    /// These presets, with each one that is not set taken from `under`.
    pub fn over(&self, under: &Presets) -> Presets {
        Presets {
            fm_deemphasis_us: self.fm_deemphasis_us.or(under.fm_deemphasis_us),
            rds: self.rds.or(under.rds),
            tv_standard: self
                .tv_standard
                .clone()
                .or_else(|| under.tv_standard.clone()),
        }
    }
}

/// One country: its ISO 3166 code, name and ITU region, the groups it uses and its presets.
///
/// For each plan kind, a country uses the groups it lists of that kind. If it lists none of a kind, it uses its ITU region's
/// groups of that kind, unless the kind is in `absent`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountryEntry {
    /// ISO 3166-1 alpha-2 code, upper case ("AU").
    pub code: String,
    /// English short name (required in the one entry of a set of files that names the country).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// ITU region, 1 to 3 (Radio Regulations Article 5; required in the one entry of a set of files that names the country).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub itu: Option<u8>,
    /// The ids of the groups it uses, the default of each kind first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// Plan kinds the country has none of (no region group is used for them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub absent: Vec<PlanKind>,
    /// Its presets (each one not set comes from its ITU region).
    #[serde(default, skip_serializing_if = "Presets::is_empty")]
    pub presets: Presets,
    /// The groups and presets have been compared with the sources.
    #[serde(default, skip_serializing_if = "is_false")]
    pub verified: bool,
    /// Why the groups and presets are not verified (required when the entry has groups, absent kinds or presets and
    /// `verified` is false).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unverified_reason: String,
    /// The documents that say which plans the country uses and its presets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<Source>,
    /// A note.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// One ITU region: the groups and presets of a country that provides none of its own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegionEntry {
    /// The ITU region, 1 to 3.
    pub itu: u8,
    /// Its name.
    pub name: String,
    /// The ids of the groups used there, the default of each kind first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// Its presets.
    #[serde(default, skip_serializing_if = "Presets::is_empty")]
    pub presets: Presets,
    /// A note.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A problem in a plan file: the file, the JSON path of the value and the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataError {
    /// The file name, as given to [`parse`] or [`check`].
    pub file: String,
    /// The JSON path of the value (`$.groups[3].bands[1].end_hz`).
    pub path: String,
    /// What is wrong, in plain words.
    pub reason: String,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.file, self.path, self.reason)
    }
}

/// Reads one plan file. A value of the wrong type, an unknown key or a missing key returns a [`DataError`] with its JSON path.
pub fn parse(file: &str, text: &str) -> Result<PlanFile, DataError> {
    let de = &mut serde_json::Deserializer::from_str(text);
    let plan: PlanFile = serde_path_to_error::deserialize(de).map_err(|e| {
        let path = e.path().to_string();
        DataError {
            file: file.to_string(),
            path: if path == "." {
                "$".into()
            } else {
                format!("$.{path}")
            },
            reason: e.into_inner().to_string(),
        }
    })?;
    if plan.format != FORMAT_VERSION {
        return Err(DataError {
            file: file.to_string(),
            path: "$.format".into(),
            reason: format!(
                "the format version is {}; this library reads version {FORMAT_VERSION}",
                plan.format
            ),
        });
    }
    Ok(plan)
}

/// What a set of files can name besides its own contents (for a user file: the loaded groups and countries).
#[derive(Clone, Debug, Default)]
pub struct Known {
    /// Group ids defined elsewhere.
    pub groups: BTreeSet<String>,
    /// Country codes defined elsewhere.
    pub countries: BTreeSet<String>,
}

/// True for an id of 1 to 64 lower-case letters, digits, '_' or '-'.
pub fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn ok_hz(hz: f64) -> bool {
    hz.is_finite() && hz > 0.0 && hz < MAX_HZ
}

/// Collects problems with their paths.
struct Problems<'a> {
    file: &'a str,
    out: Vec<DataError>,
}

impl Problems<'_> {
    fn add(&mut self, path: &str, reason: impl Into<String>) {
        self.out.push(DataError {
            file: self.file.to_string(),
            path: path.to_string(),
            reason: reason.into(),
        });
    }
}

/// How many channels a band expands to.
pub fn channel_count(b: &Band) -> usize {
    let numbered = b
        .numbering
        .as_ref()
        .map_or(0, |n| (n.last.saturating_sub(n.first) as usize) + 1);
    numbered + b.channels.len()
}

/// The centre of channel `n` of a numbering, Hz.
pub fn numbered_centre(n: &Numbering, number: u32) -> f64 {
    n.first_centre_hz + f64::from(number - n.first) * n.step_hz + n.offset_hz
}

/// `s` is a date as "YYYY", "YYYY-MM" or "YYYY-MM-DD", with a month 1 to 12 and a day 1 to 31.
pub fn is_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let num = |t: &str, len: usize, lo: u32, hi: u32| {
        t.len() == len
            && t.bytes().all(|b| b.is_ascii_digit())
            && t.parse::<u32>().is_ok_and(|v| (lo..=hi).contains(&v))
    };
    match parts.as_slice() {
        [y] => num(y, 4, 1800, 9999),
        [y, m] => num(y, 4, 1800, 9999) && num(m, 2, 1, 12),
        [y, m, d] => num(y, 4, 1800, 9999) && num(m, 2, 1, 12) && num(d, 2, 1, 31),
        _ => false,
    }
}

fn check_source(p: &mut Problems, path: &str, s: &Source) {
    if s.publisher.trim().is_empty() {
        p.add(&format!("{path}.publisher"), "the source needs a publisher");
    }
    if s.title.trim().is_empty() {
        p.add(&format!("{path}.title"), "the source needs a title");
    }
    if s.date.trim().is_empty() {
        p.add(
            &format!("{path}.date"),
            "the source needs a date or version",
        );
    }
}

fn check_band(p: &mut Problems, path: &str, b: &Band) {
    if b.name.trim().is_empty() {
        p.add(&format!("{path}.name"), "the band needs a name");
    }
    if !(ok_hz(b.start_hz) && ok_hz(b.end_hz) && b.start_hz < b.end_hz) {
        p.add(
            &format!("{path}.end_hz"),
            "the edges must be frequencies above 0 and below 300 GHz, the lower edge first",
        );
        return;
    }
    for (i, m) in b.modes.iter().enumerate() {
        if !is_id(m) {
            p.add(
                &format!("{path}.modes[{i}]"),
                format!("{m:?} is not a stack id (lower-case letters, digits, '_' or '-')"),
            );
        }
    }
    if !b.in_use && b.note.trim().is_empty() {
        p.add(
            &format!("{path}.note"),
            "a band that is not in use needs a note that says what uses it now",
        );
    }
    let inside = |c: f64, w: f64| c - w / 2.0 >= b.start_hz - 1.0 && c + w / 2.0 <= b.end_hz + 1.0;
    let width_ok = |w: f64| w.is_finite() && w > 0.0 && w <= 100e6;
    if let Some(w) = b.channel_width_hz {
        if !width_ok(w) {
            p.add(
                &format!("{path}.channel_width_hz"),
                "the channel width must be above 0 and at most 100 MHz",
            );
        }
    }
    if let Some(n) = &b.numbering {
        let np = format!("{path}.numbering");
        if n.last < n.first {
            p.add(&format!("{np}.last"), "the last number is below the first");
        } else if !(n.step_hz.is_finite() && n.step_hz > 0.0) {
            p.add(&format!("{np}.step_hz"), "the step must be above 0");
        } else if !n.offset_hz.is_finite() {
            p.add(&format!("{np}.offset_hz"), "the offset is not a number");
        } else if let Some(w) = b.channel_width_hz.filter(|w| width_ok(*w)) {
            for number in [n.first, n.last] {
                if !inside(numbered_centre(n, number), w) {
                    p.add(&np, format!("channel {number} is outside the band edges"));
                }
            }
        } else {
            p.add(
                &format!("{path}.channel_width_hz"),
                "numbered channels need a channel width",
            );
        }
    }
    for (i, c) in b.channels.iter().enumerate() {
        let cp = format!("{path}.channels[{i}]");
        if c.name.trim().is_empty() {
            p.add(&format!("{cp}.name"), "the channel needs a name");
        }
        let Some(w) = c.width_hz.or(b.channel_width_hz) else {
            p.add(
                &format!("{cp}.width_hz"),
                "the channel needs a width (here or in channel_width_hz)",
            );
            continue;
        };
        if !width_ok(w) {
            p.add(
                &format!("{cp}.width_hz"),
                "the width must be above 0 and at most 100 MHz",
            );
        } else if !ok_hz(c.centre_hz) || !inside(c.centre_hz, w) {
            p.add(
                &format!("{cp}.centre_hz"),
                "the channel is outside the band edges",
            );
        }
        if let Some(pair) = c.pair_hz {
            if !ok_hz(pair) {
                p.add(&format!("{cp}.pair_hz"), "the pair is not a frequency");
            }
        }
    }
    if let Some(r) = &b.raster {
        let rp = format!("{path}.raster");
        if b.numbering.is_some() || !b.channels.is_empty() {
            p.add(
                &rp,
                "a band with channels has no tuning raster (the channels are its tuning points)",
            );
        }
        if !(r.step_hz.is_finite() && r.step_hz > 0.0) {
            p.add(&format!("{rp}.step_hz"), "the step must be above 0");
        } else if (b.end_hz - b.start_hz) / r.step_hz > MAX_RASTER_POINTS as f64 {
            p.add(
                &format!("{rp}.step_hz"),
                format!("more than {MAX_RASTER_POINTS} tuning points"),
            );
        }
        if !(r.first_hz >= b.start_hz - 1.0 && r.first_hz <= b.end_hz + 1.0) {
            p.add(
                &format!("{rp}.first_hz"),
                "the first tuning point is outside the band edges",
            );
        }
    }
    if let Some(c) = &b.cellular {
        check_cellular(p, path, b, c);
    }
}

fn check_cellular(p: &mut Problems, path: &str, b: &Band, c: &Cellular) {
    let cp = format!("{path}.cellular");
    if b.numbering.is_some() || !b.channels.is_empty() || b.raster.is_some() {
        p.add(
            &cp,
            "a cellular band has no other channels or raster (its channel numbers are its tuning points)",
        );
    }
    if c.numbers.is_empty() {
        p.add(
            &format!("{cp}.numbers"),
            "a cellular band needs its channel numbers",
        );
    }
    for (i, r) in c.numbers.iter().enumerate() {
        let rp = format!("{cp}.numbers[{i}]");
        if r.last < r.first || r.step == 0 || !(r.last - r.first).is_multiple_of(r.step) {
            p.add(
                &rp,
                "the numbers must run from first to last in whole steps above 0",
            );
        } else if !(r.hz_per_number.is_finite() && r.hz_per_number > 0.0) {
            p.add(
                &format!("{rp}.hz_per_number"),
                "the spacing must be above 0",
            );
        } else {
            for n in [r.first, r.last] {
                let hz = r.first_hz + f64::from(n - r.first) * r.hz_per_number;
                if !(hz >= b.start_hz - 1.0 && hz <= b.end_hz + 1.0) {
                    p.add(&rp, format!("number {n} is outside the band edges"));
                }
            }
        }
    }
    for (i, s) in c.sync.iter().enumerate() {
        let sp = format!("{cp}.sync[{i}]");
        if c.system != CellSystem::Nr {
            p.add(&sp, "only NR bands have a GSCN raster");
        }
        if s.last < s.first || s.step == 0 || !(s.last - s.first).is_multiple_of(s.step) {
            p.add(
                &sp,
                "the GSCNs must run from first to last in whole steps above 0",
            );
        }
        if ![15, 30, 120, 240].contains(&s.scs_khz) {
            p.add(
                &format!("{sp}.scs_khz"),
                "the SSB spacing must be 15, 30, 120 or 240 kHz",
            );
        }
    }
}

fn check_presets(p: &mut Problems, path: &str, s: &Presets) {
    if let Some(t) = s.fm_deemphasis_us {
        if !(t.is_finite() && (0.0..=1000.0).contains(&t)) {
            p.add(
                &format!("{path}.fm_deemphasis_us"),
                "the de-emphasis must be 0 to 1000 microseconds",
            );
        }
    }
    if let Some(m) = &s.tv_standard {
        if !is_id(m) {
            p.add(
                &format!("{path}.tv_standard"),
                format!("{m:?} is not a stack id"),
            );
        }
    }
}

/// Checks a set of plan files together (their groups, countries and regions name each other). Every problem is listed, each
/// with its file and JSON path. An empty list means the files are good.
pub fn check(files: &[(String, PlanFile)], known: &Known) -> Vec<DataError> {
    let mut out = Vec::new();
    // Every group id, and the kind of each, across the files and what is known elsewhere.
    let mut ids: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut kinds: BTreeMap<&str, PlanKind> = BTreeMap::new();
    let mut codes: BTreeSet<String> = known.countries.clone();
    for (file, f) in files {
        let mut p = Problems {
            file,
            out: Vec::new(),
        };
        for (i, g) in f.groups.iter().enumerate() {
            let path = format!("$.groups[{i}]");
            if let Some((other_file, other_path)) = ids.get(&g.id) {
                p.add(
                    &format!("{path}.id"),
                    format!(
                        "the id {:?} is used twice (also {other_file}: {other_path})",
                        g.id
                    ),
                );
            }
            ids.insert(g.id.clone(), (file.clone(), path));
            kinds.insert(&g.id, g.kind);
        }
        for c in &f.countries {
            codes.insert(c.code.clone());
        }
        out.extend(p.out);
    }
    for (file, f) in files {
        let mut p = Problems {
            file,
            out: Vec::new(),
        };
        let exists = |id: &str| ids.contains_key(id) || known.groups.contains(id);
        for (i, g) in f.groups.iter().enumerate() {
            let path = format!("$.groups[{i}]");
            if !is_id(&g.id) {
                p.add(
                    &format!("{path}.id"),
                    "the id must be 1 to 64 lower-case letters, digits, '_' or '-'",
                );
            }
            if g.name.trim().is_empty() {
                p.add(&format!("{path}.name"), "the group needs a name");
            }
            check_source(&mut p, &format!("{path}.source"), &g.source);
            for (j, s) in g.also.iter().enumerate() {
                check_source(&mut p, &format!("{path}.also[{j}]"), s);
            }
            if !g.verified && g.unverified_reason.trim().is_empty() {
                p.add(
                    &format!("{path}.unverified_reason"),
                    "a group that is not verified must say why",
                );
            }
            if !g.ended.is_empty() && !is_date(&g.ended) {
                p.add(
                    &format!("{path}.ended"),
                    "the end date must be YYYY, YYYY-MM or YYYY-MM-DD",
                );
            }
            if g.bands.is_empty() && g.include.is_empty() {
                p.add(&format!("{path}.bands"), "the group has no bands");
            }
            for (j, inc) in g.include.iter().enumerate() {
                if inc == &g.id {
                    p.add(
                        &format!("{path}.include[{j}]"),
                        "a group cannot include itself",
                    );
                } else if !exists(inc) {
                    p.add(
                        &format!("{path}.include[{j}]"),
                        format!("there is no group {inc:?}"),
                    );
                } else if kinds.get(inc.as_str()).is_some_and(|k| *k != g.kind) {
                    p.add(
                        &format!("{path}.include[{j}]"),
                        format!("the group {inc:?} is of another kind"),
                    );
                }
            }
            if let Some(a) = &g.analog {
                if !(a.vision_above_lower_hz.is_finite()
                    && a.vision_above_lower_hz >= 0.0
                    && a.sound_spacing_hz.is_finite()
                    && a.sound_spacing_hz > 0.0)
                {
                    p.add(
                        &format!("{path}.analog"),
                        "the carrier offsets must be numbers above 0",
                    );
                }
            }
            let mut total = 0usize;
            for (j, b) in g.bands.iter().enumerate() {
                check_band(&mut p, &format!("{path}.bands[{j}]"), b);
                total += channel_count(b);
            }
            if total > MAX_CHANNELS {
                p.add(
                    &format!("{path}.bands"),
                    format!("more than {MAX_CHANNELS} channels"),
                );
            }
        }
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (i, c) in f.countries.iter().enumerate() {
            let path = format!("$.countries[{i}]");
            if c.code.len() != 2 || !c.code.chars().all(|ch| ch.is_ascii_uppercase()) {
                p.add(
                    &format!("{path}.code"),
                    format!("{:?} is not an upper-case ISO 3166 alpha-2 code", c.code),
                );
            }
            if !seen.insert(&c.code) {
                p.add(
                    &format!("{path}.code"),
                    format!("the country {} is listed twice", c.code),
                );
            }
            if let Some(itu) = c.itu {
                if !(1..=3).contains(&itu) {
                    p.add(&format!("{path}.itu"), "the ITU region is 1, 2 or 3");
                }
            }
            if !known.countries.contains(&c.code) && (c.name.is_empty() || c.itu.is_none()) {
                p.add(
                    &format!("{path}.code"),
                    format!(
                        "{} is not a known country: give its name and ITU region",
                        c.code
                    ),
                );
            }
            for (j, gid) in c.groups.iter().enumerate() {
                if !exists(gid) {
                    p.add(
                        &format!("{path}.groups[{j}]"),
                        format!("there is no group {gid:?}"),
                    );
                } else if kinds
                    .get(gid.as_str())
                    .is_some_and(|k| c.absent.contains(k))
                {
                    p.add(
                        &format!("{path}.groups[{j}]"),
                        format!("the group {gid:?} is of a kind the country lists as absent"),
                    );
                }
            }
            check_presets(&mut p, &format!("{path}.presets"), &c.presets);
            for (j, s) in c.sources.iter().enumerate() {
                check_source(&mut p, &format!("{path}.sources[{j}]"), s);
            }
            let says_something =
                !c.groups.is_empty() || !c.absent.is_empty() || !c.presets.is_empty();
            if says_something && !c.verified && c.unverified_reason.trim().is_empty() {
                p.add(
                    &format!("{path}.unverified_reason"),
                    "an entry that gives groups or presets and is not verified must say why",
                );
            }
            if says_something && c.verified && c.sources.is_empty() {
                p.add(
                    &format!("{path}.sources"),
                    "a verified entry must name its sources",
                );
            }
        }
        let mut regions: BTreeSet<u8> = BTreeSet::new();
        for (i, r) in f.regions.iter().enumerate() {
            let path = format!("$.regions[{i}]");
            if !(1..=3).contains(&r.itu) {
                p.add(&format!("{path}.itu"), "the ITU region is 1, 2 or 3");
            }
            if !regions.insert(r.itu) {
                p.add(
                    &format!("{path}.itu"),
                    format!("ITU region {} is listed twice", r.itu),
                );
            }
            for (j, gid) in r.groups.iter().enumerate() {
                if !exists(gid) {
                    p.add(
                        &format!("{path}.groups[{j}]"),
                        format!("there is no group {gid:?}"),
                    );
                }
            }
            check_presets(&mut p, &format!("{path}.presets"), &r.presets);
        }
        out.extend(p.out);
    }
    // Include cycles, across every file.
    let includes: BTreeMap<&str, &Vec<String>> = files
        .iter()
        .flat_map(|(_, f)| f.groups.iter().map(|g| (g.id.as_str(), &g.include)))
        .collect();
    for (file, f) in files {
        for (i, g) in f.groups.iter().enumerate() {
            let mut stack: Vec<&str> = g.include.iter().map(String::as_str).collect();
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            while let Some(id) = stack.pop() {
                if id == g.id {
                    out.push(DataError {
                        file: file.clone(),
                        path: format!("$.groups[{i}].include"),
                        reason: format!(
                            "the group {:?} includes itself through other groups",
                            g.id
                        ),
                    });
                    break;
                }
                if seen.insert(id) {
                    if let Some(next) = includes.get(id) {
                        stack.extend(next.iter().map(String::as_str));
                    }
                }
            }
        }
    }
    out
}
