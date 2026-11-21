//! The data of the LoRa stacks under `lora/`: the Meshtastic regions and the region of each country, the band plans of the
//! mesh stacks, the MeshCore presets, the amateur LoRa channels, the LoRaWAN plans and the duty-cycle limits. Each
//! `*_settings` call returns the plain settings of a stack for one named row; the stack does not know the row.
//!
//! The lists that the settings hold:
//!
//! | Setting | Entry |
//! |---|---|
//! | `channels` | `label=protocol/Hz/bandwidth Hz/SF/CR[/preamble/sync]`, entries separated by `;` |
//! | `lw_channels` | `Hz/bandwidth Hz/SF-SF/flags` with the flags `u`, `d` and `j`, entries separated by `;` |
//! | `duty_bands` | `low Hz-high Hz=percent`, entries separated by `;` |

use crate::data::{Data, Table};
use crate::error::{DataError, PresetError};
use sdrkit_core::Value;
pub use sdrkit_proto_meshtastic::SlotRow;
use sdrkit_proto_meshtastic::{self as meshtastic, SlotBand, SlotChoice, SlotGrid};

/// A Meshtastic region: the band in which the primary channel chooses its slot.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshtasticRegion {
    /// The name ("EU_868").
    pub name: String,
    /// The lower edge, Hz.
    pub start_hz: f64,
    /// The upper edge, Hz.
    pub end_hz: f64,
    /// The presets that have their own frequency in the region: preset name and frequency, Hz.
    pub fixed: Vec<(String, f64)>,
}

/// A band plan of the mesh stacks.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshBandPlan {
    /// The short name, the key of the row ("EU 868").
    pub plan: String,
    /// The name shown.
    pub name: String,
    /// The dial, Hz.
    pub dial_hz: f64,
    /// The stacks that offer the plan.
    pub stacks: Vec<String>,
    /// The Meshtastic region of the plan.
    pub meshtastic_region: String,
    /// The group of MeshCore presets heard.
    pub meshcore_group: String,
    /// The row of `meshcore_defaults.csv` for a group without presets.
    pub meshcore_default: String,
    /// The dial of the stack `meshcore`, Hz.
    pub meshcore_dial_hz: f64,
    /// The LoRaWAN plan heard, empty for none.
    pub lorawan_plan: String,
}

/// A LoRa channel of a list: a MeshCore preset or an amateur channel.
#[derive(Clone, Debug, PartialEq)]
pub struct LoraChannel {
    /// The protocol key: `meshcore`, `loraaprs` or `meshcom`.
    pub protocol: String,
    /// The name.
    pub label: String,
    /// The part of the world of a MeshCore preset, empty for an amateur channel.
    pub group: String,
    /// The centre, Hz.
    pub frequency_hz: f64,
    /// The bandwidth, Hz.
    pub bandwidth_hz: f64,
    /// The spreading factor.
    pub sf: u8,
    /// The coding rate, 5 to 8.
    pub cr: u8,
    /// The preamble, when the row gives one.
    pub preamble: Option<u16>,
    /// The sync word, when the row gives one.
    pub sync: Option<u8>,
    /// The note of the row.
    pub note: String,
}

impl LoraChannel {
    /// The entry of the setting `channels`.
    pub fn entry(&self) -> String {
        let mut e = format!(
            "{}={}/{}/{}/{}/{}",
            self.label, self.protocol, self.frequency_hz, self.bandwidth_hz, self.sf, self.cr
        );
        if let Some(p) = self.preamble {
            e += &format!("/{p}");
            if let Some(s) = self.sync {
                e += &format!("/0x{s:02X}");
            }
        }
        e
    }
}

/// A LoRaWAN plan: its centre and its FSK channel.
#[derive(Clone, Debug, PartialEq)]
pub struct LorawanPlan {
    /// The name ("EU868").
    pub plan: String,
    /// The centre, Hz.
    pub centre_hz: f64,
    /// The FSK channel, Hz, if the plan has one.
    pub fsk_hz: Option<f64>,
}

/// A LoRaWAN channel.
#[derive(Clone, Debug, PartialEq)]
pub struct LorawanChannel {
    /// The centre, Hz.
    pub frequency_hz: f64,
    /// The bandwidth, Hz.
    pub bandwidth_hz: f64,
    /// The lowest spreading factor.
    pub sf_min: u8,
    /// The highest spreading factor.
    pub sf_max: u8,
    /// Devices send here.
    pub uplink: bool,
    /// Gateways answer here.
    pub downlink: bool,
    /// A default (join) channel.
    pub default: bool,
}

impl LorawanChannel {
    /// The entry of the setting `lw_channels`.
    pub fn entry(&self) -> String {
        let flags: String = [
            (self.uplink, 'u'),
            (self.downlink, 'd'),
            (self.default, 'j'),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, c)| *c)
        .collect();
        format!(
            "{}/{}/{}-{}/{flags}",
            self.frequency_hz, self.bandwidth_hz, self.sf_min, self.sf_max
        )
    }
}

/// A band with a duty-cycle limit.
#[derive(Clone, Debug, PartialEq)]
pub struct DutyLimit {
    /// The name.
    pub name: String,
    /// The lower edge, Hz.
    pub low_hz: f64,
    /// The upper edge, Hz.
    pub high_hz: f64,
    /// The limit, percent of the last hour.
    pub percent: f64,
}

/// The plain settings of a stack for one row: by key, and the frequency to tune to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoraSettings {
    /// The frequency to tune to, Hz, when the row has one.
    pub frequency_hz: Option<f64>,
    /// The settings of the stack, by key.
    pub settings: Vec<(String, Value)>,
}

impl LoraSettings {
    fn hz(&mut self, key: &str, hz: f64) {
        self.settings.push((key.to_owned(), Value::Hz(hz)));
    }
    fn text(&mut self, key: &str, entries: &[String]) {
        self.settings
            .push((key.to_owned(), Value::Text(entries.join("; "))));
    }
}

/// A frequency in MHz of a row as Hz, rounded to the hertz.
fn mhz(t: &Table, line: usize, r: &[String], name: &str) -> Result<f64, DataError> {
    Ok((t.required(line, r, name)? * 1e6).round())
}

/// A small whole number of a row.
fn small(t: &Table, line: usize, r: &[String], name: &str) -> Result<u8, DataError> {
    u8::try_from(t.count(line, r, name)?).map_err(|_| DataError::Format {
        path: t.path.clone(),
        reason: format!("line {line}: the column `{name}` is too large"),
    })
}

/// A flag (0 or 1) of a row.
fn flag(t: &Table, line: usize, r: &[String], name: &str) -> Result<bool, DataError> {
    match t.count(line, r, name)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(DataError::Format {
            path: t.path.clone(),
            reason: format!("line {line}: the column `{name}` is not 0 or 1"),
        }),
    }
}

/// A sync word of a row: "0x12" or "18". `None` for an empty field.
fn sync(t: &Table, line: usize, r: &[String], name: &str) -> Result<Option<u8>, DataError> {
    let v = t.text(r, name);
    if v.is_empty() {
        return Ok(None);
    }
    let n = match v.strip_prefix("0x") {
        Some(h) => u8::from_str_radix(h, 16).ok(),
        None => v.parse().ok(),
    };
    n.map(Some).ok_or_else(|| DataError::Format {
        path: t.path.clone(),
        reason: format!("line {line}: `{v}` in the column `{name}` is not a sync word"),
    })
}

/// The refusal of a row that the data does not hold.
fn no_row(stack: &str, row: &str) -> PresetError {
    PresetError::NoData {
        stack: stack.to_owned(),
        region: row.to_owned(),
    }
}

impl Data {
    fn lora_table(&self, file: &str, columns: &[&str]) -> Result<Table, DataError> {
        Table::read(self.folder().join("lora").join(file), columns)
    }

    /// The Meshtastic regions of `lora/meshtastic_regions.csv`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
    pub fn meshtastic_regions(&self) -> Result<Vec<MeshtasticRegion>, DataError> {
        let t = self.lora_table(
            "meshtastic_regions.csv",
            &["region", "start_mhz", "end_mhz", "fixed"],
        )?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            let mut fixed = Vec::new();
            for e in t.text(r, "fixed").split_whitespace() {
                let parsed = e.split_once('=').and_then(|(p, f)| {
                    Some((p.to_owned(), (f.parse::<f64>().ok()? * 1e6).round()))
                });
                fixed.push(parsed.ok_or_else(|| DataError::Format {
                    path: t.path.clone(),
                    reason: format!("line {line}: `{e}` is not preset=MHz"),
                })?);
            }
            out.push(MeshtasticRegion {
                name: t.text(r, "region").to_owned(),
                start_hz: mhz(&t, *line, r, "start_mhz")?,
                end_hz: mhz(&t, *line, r, "end_mhz")?,
                fixed,
            });
        }
        Ok(out)
    }

    /// The Meshtastic region of the country `iso2` (ISO 3166-1 alpha-2, any case) from
    /// `lora/meshtastic_countries.csv`. `None` for a country that the file does not list.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn meshtastic_region_of(&self, iso2: &str) -> Result<Option<String>, DataError> {
        let t = self.lora_table("meshtastic_countries.csv", &["iso2", "region"])?;
        Ok(t.rows
            .iter()
            .find(|(_, r)| t.text(r, "iso2").eq_ignore_ascii_case(iso2.trim()))
            .map(|(_, r)| t.text(r, "region").to_owned()))
    }

    /// The settings of the Meshtastic region `region` for the primary preset `preset`: `mt_slot_start` and `mt_slot_end`,
    /// and `mt_override_freq` when the preset has its own frequency in the region.
    ///
    /// # Errors
    ///
    /// [`PresetError::NoData`] for a region that the file does not hold, [`PresetError::Data`].
    pub fn meshtastic_settings(
        &self,
        region: &str,
        preset: &str,
    ) -> Result<LoraSettings, PresetError> {
        let r = self
            .meshtastic_regions()?
            .into_iter()
            .find(|r| r.name == region)
            .ok_or_else(|| no_row("meshtastic", region))?;
        let mut s = LoraSettings::default();
        s.hz("mt_slot_start", r.start_hz);
        s.hz("mt_slot_end", r.end_hz);
        if let Some((_, hz)) = r.fixed.iter().find(|(p, _)| p == preset) {
            s.hz("mt_override_freq", *hz);
        }
        Ok(s)
    }

    /// The Meshtastic slots of the preset `preset` (empty: LongFast) in `region`, with no session, as the table `slots` of
    /// a running stack `meshtastic` lists them. `region` is a Meshtastic region ("ANZ"), else a band plan of the mesh stacks
    /// ("EU 868"), else a country (ISO 3166-1 alpha-2). The slot of `channel_name` (empty: the preset's) or of
    /// `channel_num` (from 1, 0: from the name) is current; a preset with its own frequency in the region makes the slot
    /// of that frequency current, if it is one.
    ///
    /// # Errors
    ///
    /// [`PresetError::NoData`] for a region that the data does not hold or a preset that the stack does not have,
    /// [`PresetError::ChannelNumTooHigh`] for a channel number above the slots of the band, [`PresetError::Data`].
    pub fn meshtastic_slots(
        &self,
        region: &str,
        preset: &str,
        channel_name: &str,
        channel_num: u32,
    ) -> Result<Vec<SlotRow>, PresetError> {
        let preset = if preset.is_empty() {
            "LongFast"
        } else {
            preset
        };
        let p = meshtastic::PRESETS
            .iter()
            .find(|p| p.name == preset)
            .ok_or_else(|| no_row("meshtastic", preset))?;
        let name = self.meshtastic_region_named(region)?;
        let r = self
            .meshtastic_regions()?
            .into_iter()
            .find(|r| r.name == name)
            .ok_or_else(|| no_row("meshtastic", &name))?;
        let band = SlotBand {
            start_hz: r.start_hz,
            end_hz: r.end_hz,
        };
        meshtastic::check_channel_num(p, &band, channel_num)?;
        let choice = SlotChoice {
            channel_name,
            channel_num,
            offset_hz: 0.0,
            override_hz: r
                .fixed
                .iter()
                .find(|(f, _)| f == preset)
                .map_or(0.0, |f| f.1),
        };
        Ok(SlotGrid::new(p, &band, &choice).rows().collect())
    }

    /// The Meshtastic region of `region`: a region of the file, else that of a band plan, else that of a country.
    fn meshtastic_region_named(&self, region: &str) -> Result<String, PresetError> {
        if self.meshtastic_regions()?.iter().any(|r| r.name == region) {
            return Ok(region.to_owned());
        }
        if let Some(p) = self
            .mesh_band_plans()?
            .into_iter()
            .find(|p| p.plan == region)
        {
            return Ok(p.meshtastic_region);
        }
        self.meshtastic_region_of(region)?
            .ok_or_else(|| no_row("meshtastic", region))
    }

    /// The band plans of `lora/mesh_band_plans.csv`, in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn mesh_band_plans(&self) -> Result<Vec<MeshBandPlan>, DataError> {
        let t = self.lora_table(
            "mesh_band_plans.csv",
            &[
                "plan",
                "name",
                "dial_mhz",
                "stacks",
                "meshtastic_region",
                "meshcore_group",
                "meshcore_default",
                "meshcore_dial_mhz",
                "lorawan_plan",
            ],
        )?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            out.push(MeshBandPlan {
                plan: t.text(r, "plan").to_owned(),
                name: t.text(r, "name").to_owned(),
                dial_hz: mhz(&t, *line, r, "dial_mhz")?,
                stacks: t
                    .text(r, "stacks")
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
                meshtastic_region: t.text(r, "meshtastic_region").to_owned(),
                meshcore_group: t.text(r, "meshcore_group").to_owned(),
                meshcore_default: t.text(r, "meshcore_default").to_owned(),
                meshcore_dial_hz: mhz(&t, *line, r, "meshcore_dial_mhz")?,
                lorawan_plan: t.text(r, "lorawan_plan").to_owned(),
            });
        }
        Ok(out)
    }

    /// The band plans that `stack` offers in the Meshtastic region `region`, in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn mesh_band_plans_in(
        &self,
        stack: &str,
        region: &str,
    ) -> Result<Vec<MeshBandPlan>, DataError> {
        Ok(self
            .mesh_band_plans()?
            .into_iter()
            .filter(|p| p.meshtastic_region == region && p.stacks.iter().any(|s| s == stack))
            .collect())
    }

    /// The first band plan that `stack` offers in the Meshtastic region `region`. `None` when it offers none there.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn mesh_band_plan_in(
        &self,
        stack: &str,
        region: &str,
    ) -> Result<Option<String>, DataError> {
        Ok(self
            .mesh_band_plans_in(stack, region)?
            .into_iter()
            .next()
            .map(|p| p.plan))
    }

    /// The MeshCore presets of `lora/meshcore_presets.csv`, in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn meshcore_presets(&self) -> Result<Vec<LoraChannel>, DataError> {
        let t = self.lora_table(
            "meshcore_presets.csv",
            &["title", "group", "mhz", "khz", "sf", "cr", "note"],
        )?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            out.push(LoraChannel {
                protocol: "meshcore".into(),
                label: t.text(r, "title").to_owned(),
                group: t.text(r, "group").to_owned(),
                frequency_hz: mhz(&t, *line, r, "mhz")?,
                bandwidth_hz: t.required(*line, r, "khz")? * 1e3,
                sf: small(&t, *line, r, "sf")?,
                cr: small(&t, *line, r, "cr")?,
                preamble: None,
                sync: None,
                note: t.text(r, "note").to_owned(),
            });
        }
        Ok(out)
    }

    /// The MeshCore setting of a network code from `lora/meshcore_defaults.csv`: the row of `code`, else the row `EU`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`], and [`DataError::Format`] when the file has
    /// neither row.
    pub fn meshcore_default(&self, code: &str) -> Result<LoraChannel, DataError> {
        let t = self.lora_table("meshcore_defaults.csv", &["code", "mhz", "khz", "sf", "cr"])?;
        let row = t
            .rows
            .iter()
            .find(|(_, r)| t.text(r, "code") == code)
            .or_else(|| t.rows.iter().find(|(_, r)| t.text(r, "code") == "EU"))
            .ok_or_else(|| DataError::Format {
                path: t.path.clone(),
                reason: format!("no row `{code}` and no row `EU`"),
            })?;
        let (line, r) = (row.0, &row.1);
        Ok(LoraChannel {
            protocol: "meshcore".into(),
            label: "MeshCore default".into(),
            group: String::new(),
            frequency_hz: mhz(&t, line, r, "mhz")?,
            bandwidth_hz: t.required(line, r, "khz")? * 1e3,
            sf: small(&t, line, r, "sf")?,
            cr: small(&t, line, r, "cr")?,
            preamble: None,
            sync: None,
            note: String::new(),
        })
    }

    /// The LoRa APRS and MeshCom channels of `lora/ham_presets.csv`, in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn ham_lora_channels(&self) -> Result<Vec<LoraChannel>, DataError> {
        let t = self.lora_table(
            "ham_presets.csv",
            &[
                "protocol", "name", "mhz", "khz", "sf", "cr", "preamble", "sync",
            ],
        )?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            let preamble = u16::try_from(t.count(*line, r, "preamble")?)
                .ok()
                .filter(|&p| p > 0);
            out.push(LoraChannel {
                protocol: t.text(r, "protocol").to_owned(),
                label: t.text(r, "name").to_owned(),
                group: String::new(),
                frequency_hz: mhz(&t, *line, r, "mhz")?,
                bandwidth_hz: t.required(*line, r, "khz")? * 1e3,
                sf: small(&t, *line, r, "sf")?,
                cr: small(&t, *line, r, "cr")?,
                preamble,
                sync: sync(&t, *line, r, "sync")?,
                note: String::new(),
            });
        }
        Ok(out)
    }

    /// The settings of the band plan `plan` for the mesh stack `stack`: `dial_freq` and the frequency to tune to (the
    /// MeshCore dial for `meshcore`), `mt_slot_start` and `mt_slot_end` for `mesh`, `meshtastic` and `meshcore`, `channels`
    /// with the MeshCore presets of the plan's group for `mesh` and `meshcore` and every amateur channel for `mesh`,
    /// `loraaprs` and `meshcom`, and `lw_channels` with the uplink channels of the plan's LoRaWAN plan for `mesh` and
    /// `lorawan`. A stack searches the channels of its own protocols; the others place the networks of its test signal.
    ///
    /// # Errors
    ///
    /// [`PresetError::NoData`] for a plan that the file does not hold or that the stack does not offer,
    /// [`PresetError::Data`].
    pub fn mesh_band_plan_settings(
        &self,
        plan: &str,
        stack: &str,
    ) -> Result<LoraSettings, PresetError> {
        let p = self
            .mesh_band_plans()?
            .into_iter()
            .find(|p| p.plan == plan && p.stacks.iter().any(|s| s == stack))
            .ok_or_else(|| no_row(stack, plan))?;
        let mut s = LoraSettings::default();
        let dial = if stack == "meshcore" {
            p.meshcore_dial_hz
        } else {
            p.dial_hz
        };
        s.frequency_hz = Some(dial);
        s.hz("dial_freq", dial);
        if matches!(stack, "mesh" | "meshtastic" | "meshcore") {
            let r = self.meshtastic_settings(&p.meshtastic_region, "")?;
            s.settings.extend(r.settings);
        }
        let mut channels: Vec<String> = Vec::new();
        if matches!(stack, "mesh" | "meshcore") {
            let group: Vec<LoraChannel> = self
                .meshcore_presets()?
                .into_iter()
                .filter(|c| c.group == p.meshcore_group)
                .collect();
            if group.is_empty() {
                channels.push(self.meshcore_default(&p.meshcore_default)?.entry());
            } else {
                channels.extend(group.iter().map(LoraChannel::entry));
            }
        }
        if matches!(stack, "mesh" | "loraaprs" | "meshcom") {
            channels.extend(self.ham_lora_channels()?.iter().map(LoraChannel::entry));
        }
        if !channels.is_empty() {
            s.text("channels", &channels);
        }
        if matches!(stack, "mesh" | "lorawan") && !p.lorawan_plan.is_empty() {
            let lw: Vec<String> = self
                .lorawan_channels(&p.lorawan_plan, 0, false)?
                .iter()
                .map(LorawanChannel::entry)
                .collect();
            s.text("lw_channels", &lw);
        }
        Ok(s)
    }

    /// The LoRaWAN plans of `lora/lorawan_plans.csv`, in the order of the file.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn lorawan_plans(&self) -> Result<Vec<LorawanPlan>, DataError> {
        let t = self.lora_table("lorawan_plans.csv", &["plan", "centre_mhz", "fsk_mhz"])?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            out.push(LorawanPlan {
                plan: t.text(r, "plan").to_owned(),
                centre_hz: mhz(&t, *line, r, "centre_mhz")?,
                fsk_hz: t.number(*line, r, "fsk_mhz")?.map(|f| (f * 1e6).round()),
            });
        }
        Ok(out)
    }

    /// The channels of the LoRaWAN plan `plan`, made from the rows of `lora/lorawan_channels.csv`, ascending by frequency
    /// then bandwidth. `sub_band` 1 and up keeps that sub-band of the rows that have sub-bands (0 keeps all). The channels
    /// for downlinks only are there when `downlink_only` is set.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn lorawan_channels(
        &self,
        plan: &str,
        sub_band: u8,
        downlink_only: bool,
    ) -> Result<Vec<LorawanChannel>, DataError> {
        let t = self.lora_table(
            "lorawan_channels.csv",
            &[
                "plan",
                "first_mhz",
                "step_mhz",
                "count",
                "width_khz",
                "sf_min",
                "sf_max",
                "uplink",
                "downlink",
                "default",
                "sub_band_size",
            ],
        )?;
        let mut out = Vec::new();
        for (line, r) in t.rows.iter().filter(|(_, r)| t.text(r, "plan") == plan) {
            let line = *line;
            let first = t.required(line, r, "first_mhz")?;
            let step = t.number(line, r, "step_mhz")?.unwrap_or(0.0);
            let uplink = flag(&t, line, r, "uplink")?;
            if !uplink && !downlink_only {
                continue;
            }
            let size = t.count(line, r, "sub_band_size")?;
            for n in 0..t.count(line, r, "count")? {
                if sub_band > 0 && size > 0 && n / size + 1 != u64::from(sub_band) {
                    continue;
                }
                out.push(LorawanChannel {
                    frequency_hz: ((first + step * n as f64) * 1e6).round(),
                    bandwidth_hz: t.required(line, r, "width_khz")? * 1e3,
                    sf_min: small(&t, line, r, "sf_min")?,
                    sf_max: small(&t, line, r, "sf_max")?,
                    uplink,
                    downlink: flag(&t, line, r, "downlink")?,
                    default: flag(&t, line, r, "default")?,
                });
            }
        }
        out.sort_by(|a, b| {
            a.frequency_hz
                .total_cmp(&b.frequency_hz)
                .then(a.bandwidth_hz.total_cmp(&b.bandwidth_hz))
        });
        Ok(out)
    }

    /// The settings of the LoRaWAN plan `plan` for `lorawanwide`: `dial_freq` and the frequency to tune to (the centre of
    /// the plan), `lw_channels` with every channel of the plan, those for downlinks only too, in sub-band `sub_band` (0
    /// for all), and `fsk_freq` (0 when the plan has no FSK channel).
    ///
    /// # Errors
    ///
    /// [`PresetError::NoData`] for a plan that the files do not hold, [`PresetError::Data`].
    pub fn lorawan_settings(&self, plan: &str, sub_band: u8) -> Result<LoraSettings, PresetError> {
        let p = self
            .lorawan_plans()?
            .into_iter()
            .find(|p| p.plan == plan)
            .ok_or_else(|| no_row("lorawanwide", plan))?;
        let mut s = LoraSettings {
            frequency_hz: Some(p.centre_hz),
            ..LoraSettings::default()
        };
        s.hz("dial_freq", p.centre_hz);
        let lw: Vec<String> = self
            .lorawan_channels(plan, sub_band, true)?
            .iter()
            .map(LorawanChannel::entry)
            .collect();
        s.text("lw_channels", &lw);
        s.hz("fsk_freq", p.fsk_hz.unwrap_or(0.0));
        Ok(s)
    }

    /// The duty-cycle limits of `lora/duty_limits.csv`.
    ///
    /// # Errors
    ///
    /// [`DataError::Missing`], [`DataError::Read`], [`DataError::Format`].
    pub fn lora_duty_limits(&self) -> Result<Vec<DutyLimit>, DataError> {
        let t = self.lora_table(
            "duty_limits.csv",
            &["name", "lo_mhz", "hi_mhz", "duty_percent"],
        )?;
        let mut out = Vec::new();
        for (line, r) in &t.rows {
            out.push(DutyLimit {
                name: t.text(r, "name").to_owned(),
                low_hz: mhz(&t, *line, r, "lo_mhz")?,
                high_hz: mhz(&t, *line, r, "hi_mhz")?,
                percent: t.required(*line, r, "duty_percent")?,
            });
        }
        Ok(out)
    }

    /// The setting `duty_bands` of the mesh stacks with every limit of `lora/duty_limits.csv`.
    ///
    /// # Errors
    ///
    /// [`PresetError::Data`].
    pub fn lora_duty_settings(&self) -> Result<LoraSettings, PresetError> {
        let bands: Vec<String> = self
            .lora_duty_limits()?
            .iter()
            .map(|d| format!("{}-{}={}", d.low_hz, d.high_hz, d.percent))
            .collect();
        let mut s = LoraSettings::default();
        s.text("duty_bands", &bands);
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> Data {
        Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data")).expect("the data folder")
    }

    fn text<'a>(s: &'a LoraSettings, key: &str) -> &'a str {
        match s.settings.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
            Some(Value::Text(t)) => t,
            other => panic!("{key}: {other:?}"),
        }
    }

    fn hz(s: &LoraSettings, key: &str) -> f64 {
        match s.settings.iter().find(|(k, _)| k == key).map(|(_, v)| v) {
            Some(Value::Hz(h)) => *h,
            other => panic!("{key}: {other:?}"),
        }
    }

    #[test]
    fn every_row_of_every_file_loads() {
        let d = data();
        let regions = d.meshtastic_regions().unwrap();
        assert_eq!(regions.len(), 26);
        assert!(regions.iter().all(|r| r.end_hz > r.start_hz));
        assert_eq!(
            d.meshtastic_region_of("au").unwrap().as_deref(),
            Some("ANZ")
        );
        assert_eq!(d.meshtastic_region_of("XX").unwrap(), None);
        let plans = d.mesh_band_plans().unwrap();
        assert_eq!(plans.len(), 10);
        for p in &plans {
            assert!(
                regions.iter().any(|r| r.name == p.meshtastic_region),
                "{}",
                p.plan
            );
            for stack in &p.stacks {
                let s = d.mesh_band_plan_settings(&p.plan, stack).unwrap();
                assert!(
                    s.frequency_hz.is_some_and(|f| f > 0.0),
                    "{} {stack}",
                    p.plan
                );
            }
        }
        let mc = d.meshcore_presets().unwrap();
        assert_eq!(mc.len(), 26);
        for code in [
            "NL",
            "PT",
            "EU433",
            "EU_DEPRECATED",
            "US",
            "CA",
            "AU",
            "NZ",
            "ANZ_MID",
            "EU",
        ] {
            d.meshcore_default(code).unwrap();
        }
        assert_eq!(d.ham_lora_channels().unwrap().len(), 13);
        let lw = d.lorawan_plans().unwrap();
        assert_eq!(lw.len(), 8);
        for p in &lw {
            assert!(
                !d.lorawan_channels(&p.plan, 0, true).unwrap().is_empty(),
                "{}",
                p.plan
            );
            d.lorawan_settings(&p.plan, 0).unwrap();
        }
        assert_eq!(d.lora_duty_limits().unwrap().len(), 2);
    }

    #[test]
    fn the_lorawan_plans_make_their_channels() {
        let d = data();
        let n = |p: &str, sb: u8, dl: bool| d.lorawan_channels(p, sb, dl).unwrap();
        assert_eq!(n("EU868", 0, false).len(), 9);
        assert_eq!(n("US915", 0, false).len(), 72);
        assert_eq!(n("US915", 0, true).len(), 80);
        assert_eq!(n("US915", 2, false).len(), 9);
        assert_eq!(n("AS923-1", 0, false)[0].frequency_hz, 923.2e6);
        assert_eq!(n("AS923-3", 0, false)[0].frequency_hz, 916.6e6);
        assert_eq!(n("EU433", 0, false)[0].frequency_hz, 433.175e6);
        let eu = d.lorawan_settings("EU868", 0).unwrap();
        assert_eq!(eu.frequency_hz, Some(868.3e6));
        assert_eq!(hz(&eu, "fsk_freq"), 868.8e6);
        assert!(text(&eu, "lw_channels").starts_with("867100000/125000/7-12/ud; "));
        assert!(text(&eu, "lw_channels").ends_with("869525000/125000/7-12/d"));
    }

    #[test]
    fn a_band_plan_gives_its_dial_and_channels() {
        let d = data();
        let eu = d.mesh_band_plan_settings("EU 868", "mesh").unwrap();
        assert_eq!(eu.frequency_hz, Some(869.525e6));
        assert_eq!(
            (hz(&eu, "mt_slot_start"), hz(&eu, "mt_slot_end")),
            (869.4e6, 869.65e6)
        );
        let ch = text(&eu, "channels");
        assert!(
            ch.starts_with("EU/UK (Narrow)=meshcore/869618000/62500/8/8; "),
            "{ch}"
        );
        assert!(
            ch.contains("MeshCom EU=meshcom/433175000/250000/11/6/32/0x2B"),
            "{ch}"
        );
        assert!(text(&eu, "lw_channels").contains("868100000/125000/7-12/udj"));
        let mc = d.mesh_band_plan_settings("ANZ 915", "meshcore").unwrap();
        assert_eq!(mc.frequency_hz, Some(915.8e6));
        assert!(text(&mc, "channels").starts_with("Australia=meshcore/915800000/250000/10/5"));
        assert!(d
            .mesh_band_plan_settings("LoRa APRS EU", "meshcore")
            .is_err());
        assert_eq!(
            d.mesh_band_plan_in("mesh", "ANZ").unwrap().as_deref(),
            Some("ANZ 915")
        );
        assert_eq!(d.mesh_band_plan_in("meshtastic", "JP").unwrap(), None);
        let efl = d.meshtastic_settings("EU_868", "EdgeFastLow").unwrap();
        assert_eq!(hz(&efl, "mt_override_freq"), 869_431_274.0);
        let duty = d.lora_duty_settings().unwrap();
        assert_eq!(
            text(&duty, "duty_bands"),
            "869400000-869650000=10; 868000000-868600000=1"
        );
    }
}
