//! The preset of a stack in a region: its settings, its start frequency and its channels, all from the data folder.

use crate::data::{Data, Station, StationKind};
use crate::error::{DataError, PresetError};
use crate::plans::Found;
use crate::region::Region;
use sdrkit_core::Value;
use sdrkit_data_plans::RdsVariant;
use sdrkit_util_text::csv;

/// What the user chose for one start. Each part that is set replaces that part of the preset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    /// The frequency to start on, Hz.
    pub frequency_hz: Option<f64>,
    /// The variant of the stack.
    pub submode: Option<String>,
    /// Settings of the stack, by key. A choice is the key of the choice as a text.
    pub settings: Vec<(String, Value)>,
}

/// One channel of a plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanChannel {
    /// The number or the name of the channel.
    pub label: String,
    /// The centre frequency, Hz.
    pub frequency_hz: f64,
    /// The width, Hz, if the plan has one.
    pub width_hz: Option<f64>,
}

/// What starts a stack in a region.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The stack id.
    pub stack: String,
    /// The variant of the stack, from the options.
    pub submode: Option<String>,
    /// The id of the plan or the band group that the channels come from. Empty if the region has stations and no plan.
    pub plan: String,
    /// The settings of the stack, by key: those of the region, then those of the options. A choice is the key of the
    /// choice as a text.
    pub settings: Vec<(String, Value)>,
    /// The frequency to start on, Hz.
    pub frequency_hz: f64,
    /// The width of the channel at that frequency, Hz.
    pub width_hz: Option<f64>,
    /// The known station that the frequency is of, if it is one.
    pub station: Option<Station>,
    /// The channels of the plan for the stack.
    pub channels: Vec<PlanChannel>,
}

/// `value` as the text that a host shows or parses: a text as it is, a number in its shortest form, `yes` or `no`.
pub(crate) fn value_text(value: &Value) -> String {
    match value {
        Value::Text(t) => t.clone(),
        Value::F64(x) | Value::Hz(x) => x.to_string(),
        Value::I64(n) => n.to_string(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        other => format!("{other:?}"),
    }
}

impl Preset {
    /// The settings with each value as a text, for a host that sets them through an interface of texts or of its own
    /// value type. A choice is the key of the choice.
    pub fn settings_text(&self) -> Vec<(String, String)> {
        self.settings
            .iter()
            .map(|(k, v)| (k.clone(), value_text(v)))
            .collect()
    }
}

/// How a setting of a stack takes a value of the region.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Rule {
    /// The centre frequencies of the channels of the plan, Hz, separated by ";".
    Channels(&'static str),
    /// The channel width in MHz (`7`).
    WidthMhz(&'static str),
    /// The FM de-emphasis in microseconds (`50`).
    Deemphasis(&'static str),
    /// The dial range and the raster of the FM band group, as the four settings of [`crate::fm::FmBand::settings`].
    FmBand,
    /// The RDS variant (`rds`, `rbds`).
    Rds(&'static str),
}

/// The setting keys of one stack that take values of the region, and the stations it receives.
#[derive(Debug)]
pub(crate) struct Recipe {
    stack: &'static str,
    /// The list of known stations.
    pub stations: Option<StationKind>,
    /// The standards of that list that the stack receives, in lower case without punctuation. Empty: every row.
    standards: &'static [&'static str],
    rules: &'static [Rule],
}

/// The stacks of the services with one channel raster in a country, and their settings of the region. Another stack
/// gets a frequency and channels from a plan of its own, and no settings.
const RECIPES: &[Recipe] = &[
    Recipe {
        stack: "dvbt",
        stations: Some(StationKind::DvbT),
        standards: &["dvbt"],
        rules: &[Rule::Channels("channels"), Rule::WidthMhz("bandwidth")],
    },
    Recipe {
        stack: "dvbt2",
        stations: Some(StationKind::DvbT),
        standards: &["dvbt", "dvbt2"],
        rules: &[Rule::WidthMhz("bandwidth")],
    },
    Recipe {
        stack: "fm",
        stations: Some(StationKind::Fm),
        standards: &[],
        rules: &[
            Rule::FmBand,
            Rule::Deemphasis("deemphasis"),
            Rule::Rds("rds_variant"),
        ],
    },
    Recipe {
        stack: "dab",
        stations: Some(StationKind::Dab),
        standards: &[],
        rules: &[Rule::Channels("channels")],
    },
    Recipe {
        stack: "atsc",
        stations: Some(StationKind::Atsc),
        standards: &[],
        rules: &[],
    },
];

/// The recipe of `stack`.
pub(crate) fn recipe(stack: &str) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.stack == stack)
}

impl Recipe {
    /// True if the stack receives a station of the standard `standard`. A row without a standard counts.
    fn receives(&self, standard: &str) -> bool {
        let s: String = standard
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        s.is_empty() || self.standards.is_empty() || self.standards.contains(&s.as_str())
    }
}

/// The channel of `channels` that holds `hz`: the nearest one whose width reaches it.
fn channel_at(channels: &[PlanChannel], hz: f64) -> Option<&PlanChannel> {
    channels
        .iter()
        .filter(|c| (c.frequency_hz - hz).abs() <= c.width_hz.unwrap_or(0.0) / 2.0)
        .min_by(|a, b| {
            (a.frequency_hz - hz)
                .abs()
                .total_cmp(&(b.frequency_hz - hz).abs())
        })
}

impl Data {
    /// The band plan that the mesh stack `stack` offers in the country `code`, with its settings and its dial:
    /// the first plan of the Meshtastic region of the country ([`Data::mesh_band_plan_settings`]). `None` for
    /// another stack, a country without a region, a region without a plan of the stack, or a folder without the
    /// LoRa files.
    fn mesh_plan(
        &self,
        stack: &str,
        code: &str,
    ) -> Result<Option<(String, crate::lora::LoraSettings)>, PresetError> {
        let missing = |e: DataError| match e {
            DataError::Missing { .. } => Ok(None),
            e => Err(e),
        };
        let Some(region) = self.meshtastic_region_of(code).or_else(missing)? else {
            return Ok(None);
        };
        let Some(plan) = self.mesh_band_plan_in(stack, &region).or_else(missing)? else {
            return Ok(None);
        };
        let settings = self.mesh_band_plan_settings(&plan, stack)?;
        Ok(Some((plan, settings)))
    }

    /// The preset of `stack` in `region`.
    ///
    /// The frequency is that of the options. Without one it is the strongest known station of the region's city that
    /// the stack receives (the highest signal to noise ratio, then the most reports), else the first channel that the
    /// plan of the country has for the stack. The width is that of the plan's channel at the frequency, else that of
    /// the station. A missing list of stations counts as no stations.
    ///
    /// The plan of `dvbt`, `dvbt2`, `fm`, `dab` and `atsc` is the first band group of the country whose bands list the
    /// stack. Another stack has a plan only in a band group whose id starts with its id, or in a plan in the CSV or
    /// plan JSON form that names it.
    ///
    /// A mesh stack (`meshtastic`, `meshcore`, `mesh`, `lorawan` ...) takes the first band plan of the LoRa files that
    /// it offers in the country: the dial, the slot limits and the channel lists.
    ///
    /// A stack with neither a plan nor a station (a satellite downlink) starts on the first row of its downlink lists
    /// ([`Data::downlinks`]) in the submode of the options, else on the first row of any submode, which then is the
    /// submode of the preset. A stack with none of these starts on the dial that `<family>/default_dials.csv` has for
    /// it ([`Data::dial_hz`]).
    ///
    /// The settings are those of the plan (`dvbt`, `dvbt2`, `fm`, `dab`, the mesh stacks) and those that a family file
    /// has for the stack: the dials and band edges of the weak-signal stacks, the presets of `general`, the channels
    /// of `atv` and `fpv`, the row with the code of the country for `halow`, `wifi` and `zwave`, and the first band plan
    /// that lists the country for `wisun`.
    ///
    /// # Errors
    ///
    /// [`PresetError::NoData`] if the country has no plan and the city no station for the stack,
    /// [`PresetError::Data`] for a file that cannot be read or does not have its format.
    pub fn preset(
        &self,
        stack: &str,
        region: &Region,
        options: &Options,
    ) -> Result<Preset, PresetError> {
        let region = region.of_stack(stack);
        let recipe = recipe(stack);
        let found = self.plans.find(stack, region.country(), recipe.is_some());
        let station = match recipe.and_then(|r| Some((r, r.stations?))) {
            Some((r, kind)) if region.city().is_some() => match self.known_stations(kind, region) {
                Ok(rows) => rows
                    .into_iter()
                    .filter(|s| region.is_city(&s.city) && r.receives(&s.standard))
                    .max_by(|a, b| {
                        let snr = |s: &Station| s.snr_db.unwrap_or(f64::NEG_INFINITY);
                        snr(a).total_cmp(&snr(b)).then(a.reports.cmp(&b.reports))
                    }),
                Err(DataError::Missing { .. }) => None,
                Err(e) => return Err(e.into()),
            },
            _ => None,
        };
        let (mut plan, channels) = match found {
            Some(Found { id, channels }) => (id, channels),
            None => (String::new(), Vec::new()),
        };
        // A mesh stack has a band plan of the LoRa files for the country, with its dial and its channel lists.
        let mesh = if station.is_none() && channels.is_empty() {
            self.mesh_plan(stack, region.country())?
        } else {
            None
        };
        let downlink = if station.is_none() && channels.is_empty() {
            self.downlinks(stack)?.into_iter().find(|d| {
                options
                    .submode
                    .as_deref()
                    .is_none_or(|m| m == d.submode)
            })
        } else {
            None
        };
        // A stack with none of these starts on the frequency of its row of a family file, else on its starting dial.
        let family = self.family_settings(stack, region.country())?;
        let dial = if station.is_none() && channels.is_empty() && downlink.is_none() && mesh.is_none()
        {
            let hz = match family.frequency_hz {
                Some(hz) => Some(hz),
                None => self.dial_hz(stack, options.submode.as_deref())?,
            };
            hz.map(|hz| options.frequency_hz.unwrap_or(hz))
        } else {
            None
        };
        let station = station.filter(|_| options.frequency_hz.is_none());
        let frequency_hz = options
            .frequency_hz
            .or(station.as_ref().map(|s| s.frequency_hz))
            .or(channels.first().map(|c| c.frequency_hz))
            .or(downlink.as_ref().map(|d| d.frequency_hz))
            .or(mesh.as_ref().and_then(|(_, m)| m.frequency_hz))
            .filter(|_| {
                station.is_some() || !channels.is_empty() || downlink.is_some() || mesh.is_some()
            })
            .or(dial)
            .ok_or_else(|| PresetError::NoData {
                stack: stack.to_owned(),
                region: region.to_string(),
            })?;
        let width_hz = channel_at(&channels, frequency_hz)
            .and_then(|c| c.width_hz)
            .or(station.as_ref().and_then(|s| s.width_hz))
            .or(downlink.as_ref().and_then(|d| d.width_hz));
        let presets = self.plans.presets(region.country());
        let text = |key: &str, v: String| (key.to_owned(), Value::Text(v));
        let mut settings: Vec<(String, Value)> = Vec::new();
        if let Some((id, m)) = mesh {
            plan = id;
            settings = m.settings;
        }
        settings.extend(family.settings);
        for rule in recipe.map_or(&[][..], |r| r.rules) {
            if let Rule::FmBand = rule {
                let band = self.plans.group(&plan).and_then(crate::fm::FmBand::of);
                settings.extend(band.iter().flat_map(crate::fm::FmBand::settings));
                continue;
            }
            let setting = match *rule {
                Rule::FmBand => None,
                Rule::Channels(key) => (!channels.is_empty()).then(|| {
                    let hz: Vec<String> = channels
                        .iter()
                        .map(|c| format!("{:.0}", c.frequency_hz))
                        .collect();
                    text(key, hz.join(";"))
                }),
                Rule::WidthMhz(key) => width_hz.map(|w| text(key, csv::number(w / 1e6))),
                Rule::Deemphasis(key) => presets
                    .fm_deemphasis_us
                    .map(|us| text(key, csv::number(us))),
                Rule::Rds(key) => presets.rds.map(|v| {
                    let variant = match v {
                        RdsVariant::Rds => "rds",
                        RdsVariant::Rbds => "rbds",
                    };
                    text(key, variant.to_owned())
                }),
            };
            settings.extend(setting);
        }
        for (key, value) in &options.settings {
            settings.retain(|(k, _)| k != key);
            settings.push((key.clone(), value.clone()));
        }
        Ok(Preset {
            stack: stack.to_owned(),
            submode: options
                .submode
                .clone()
                .or(downlink.map(|d| d.submode)),
            plan,
            settings,
            frequency_hz,
            width_hz,
            station,
            channels,
        })
    }
}
