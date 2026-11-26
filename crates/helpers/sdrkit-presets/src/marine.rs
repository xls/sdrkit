//! Marine data files: the VHF channel plans of `marine/vhf_channels.csv`, and the radiofax stations and broadcast times
//! of `hfdata/wefax_stations.csv` and `hfdata/wefax_broadcasts.csv`.
//!
//! [`MarinePlan::settings`] gives the stacks `marinevhf` and `atis` the setting `plan` of one plan.
//! [`WefaxStation::dials_hz`] and [`WefaxBroadcast::settings`] give the tuning and the settings of the stack `marine`
//! for a radiofax broadcast.

use crate::data::{Data, Table};
use crate::error::DataError;
use sdrkit_core::Value;
use std::fmt::Write as _;
use std::path::Path;

/// The path of the channel plans in the data folder.
pub const VHF_CHANNELS: &str = "marine/vhf_channels.csv";
/// The path of the marine VHF plan of each country in the data folder.
pub const VHF_PLAN_COUNTRIES: &str = "marine/vhf_plan_countries.csv";
/// The plan of a country that [`VHF_PLAN_COUNTRIES`] does not list.
pub const DEFAULT_VHF_PLAN: &str = "INT";
/// The path of the radiofax stations in the data folder.
pub const WEFAX_STATIONS: &str = "hfdata/wefax_stations.csv";
/// The path of the radiofax broadcast times in the data folder.
pub const WEFAX_BROADCASTS: &str = "hfdata/wefax_broadcasts.csv";
/// The USB dial of a radiofax carrier is this far below the carrier, Hz.
pub const WEFAX_DIAL_OFFSET_HZ: f64 = 1900.0;

/// One channel of a marine VHF plan.
#[derive(Clone, Debug, PartialEq)]
pub struct MarineChannel {
    /// The label ("16", "22A", "87B").
    pub label: String,
    /// The channel number.
    pub number: u32,
    /// The letter after the number ("A", "B"), empty for none.
    pub suffix: String,
    /// Where a ship transmits, Hz.
    pub ship_hz: f64,
    /// Where a coast station transmits, Hz. Equal to `ship_hz` for a simplex channel.
    pub coast_hz: f64,
    /// `voice`, `dsc`, `ais` or `long_range_ais`.
    pub use_: String,
    /// The uses of the channel (`distress`, `calling`, `port`, ...).
    pub flags: Vec<String>,
    /// The most power, W.
    pub power_w: f64,
    /// A local name of the channel, empty for none.
    pub name: String,
}

impl MarineChannel {
    /// Simplex (one frequency).
    pub fn simplex(&self) -> bool {
        (self.ship_hz - self.coast_hz).abs() < 1.0
    }
    /// The frequencies to listen on (Hz): both halves of a duplex pair, or the one simplex frequency.
    pub fn listen_hz(&self) -> Vec<f64> {
        if self.simplex() {
            vec![self.ship_hz]
        } else {
            vec![self.ship_hz, self.coast_hz]
        }
    }
    /// A voice channel.
    pub fn voice(&self) -> bool {
        self.use_ == "voice"
    }
    /// The label and the uses: "16 Distress, safety, calling".
    pub fn title(&self) -> String {
        let mut uses: Vec<&str> = self
            .flags
            .iter()
            .map(|f| match f.as_str() {
                "distress" => "Distress, safety",
                "calling" => "calling",
                "sar" => "SAR",
                "bridge" => "bridge to bridge",
                "port" => "port operations",
                "vts" => "VTS",
                "intership" => "intership",
                "public" => "public correspondence",
                "safety_broadcast" => "safety broadcasts",
                "government" => "government",
                "low_power" => "low power",
                _ => "",
            })
            .filter(|u| !u.is_empty())
            .collect();
        if uses.is_empty() {
            uses.push(match self.use_.as_str() {
                "dsc" => "DSC",
                "ais" => "AIS",
                "long_range_ais" => "long-range AIS",
                _ => "voice",
            });
        }
        format!("{} {}", self.label, uses.join(", "))
    }
}

/// A marine VHF channel plan: the rows of one value of the column `plan`.
#[derive(Clone, Debug, PartialEq)]
pub struct MarinePlan {
    /// The id ("INT", "USA").
    pub id: String,
    /// The name ("International (ITU)").
    pub name: String,
    /// The channels in the order of the file.
    pub channels: Vec<MarineChannel>,
}

impl MarinePlan {
    /// By number and suffix. A number with only lettered variants returns the first variant.
    pub fn by_number(&self, n: u32, suffix: &str) -> Option<&MarineChannel> {
        let s = suffix.to_ascii_uppercase();
        self.channels
            .iter()
            .find(|c| c.number == n && c.suffix == s)
            .or_else(|| {
                s.is_empty()
                    .then(|| self.channels.iter().find(|c| c.number == n))
                    .flatten()
            })
    }
    /// By label text ("16", "ch 16", "Ch.16", "22a", "VHF 87B"; case and a leading "ch" ignored).
    pub fn by_label(&self, text: &str) -> Option<&MarineChannel> {
        let mut t = text.trim().to_ascii_uppercase();
        for p in ["VHF", "CHANNEL", "CH.", "CH"] {
            if let Some(r) = t.strip_prefix(p) {
                t = r.trim().to_string();
            }
        }
        let t = t.trim_start_matches('.').trim();
        let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
        let suffix = t[digits.len()..].trim();
        self.by_number(digits.parse().ok()?, suffix)
    }
    /// By frequency (ship or coast side, within 0.5 kHz): the channel and whether it is the coast side.
    pub fn by_frequency(&self, hz: f64) -> Option<(&MarineChannel, bool)> {
        let near = |f: f64| (f - hz).abs() <= 500.0;
        self.channels
            .iter()
            .find(|c| near(c.ship_hz))
            .map(|c| (c, false))
            .or_else(|| {
                self.channels
                    .iter()
                    .find(|c| near(c.coast_hz))
                    .map(|c| (c, true))
            })
    }
    /// The voice channels in plan order.
    pub fn voice_channels(&self) -> Vec<&MarineChannel> {
        self.channels.iter().filter(|c| c.voice()).collect()
    }
    /// Every voice frequency, ascending (Hz).
    pub fn voice_frequencies(&self) -> Vec<f64> {
        sorted(
            self.voice_channels()
                .iter()
                .flat_map(|c| c.listen_hz())
                .collect(),
        )
    }
    /// Every frequency, ascending (Hz).
    pub fn all_frequencies(&self) -> Vec<f64> {
        sorted(
            self.channels
                .iter()
                .flat_map(MarineChannel::listen_hz)
                .collect(),
        )
    }
    /// The next (or previous) voice channel after the one labelled `label`, wrapping.
    pub fn step_voice(&self, label: &str, forward: bool) -> Option<&MarineChannel> {
        let v = self.voice_channels();
        let i = v.iter().position(|c| c.label == label)?;
        let n = v.len();
        Some(
            v[if forward {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            }],
        )
    }
    /// The channel named in a DSC call's text ("VHF ch 16", "VHF ch 1016 (ship simplex)", "16", "ch 06"; a designation of
    /// 100 or more keeps its last two digits).
    pub fn from_dsc_text(&self, text: &str) -> Option<&MarineChannel> {
        let start = text.find(|c: char| c.is_ascii_digit())?;
        let digits: String = text[start..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let mut n: u32 = digits.parse().ok()?;
        if n >= 100 {
            n %= 100;
        }
        self.by_number(n, "")
    }
    /// The text of the stack setting `plan`: `label=ship_hz` or `label=ship_hz/coast_hz` entries separated by `;`, a
    /// channel that is not a voice channel marked `:data`.
    pub fn plan_text(&self) -> String {
        let mut out = String::new();
        for c in &self.channels {
            if !out.is_empty() {
                out.push(';');
            }
            let _ = write!(out, "{}={}", c.label, c.ship_hz.round());
            if !c.simplex() {
                let _ = write!(out, "/{}", c.coast_hz.round());
            }
            if !c.voice() {
                out.push_str(":data");
            }
        }
        out
    }
    /// The settings of the stacks `marinevhf` and `atis` for this plan.
    pub fn settings(&self) -> Vec<(String, Value)> {
        vec![("plan".to_owned(), Value::Text(self.plan_text()))]
    }
}

/// `v` ascending, without frequencies within 1 Hz of the one before.
fn sorted(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 1.0);
    v
}

/// One radiofax station.
#[derive(Clone, Debug, PartialEq)]
pub struct WefaxStation {
    /// The id that the broadcast times name ("DDK").
    pub id: String,
    /// The name.
    pub name: String,
    /// The call signs.
    pub callsigns: Vec<String>,
    /// The country.
    pub country: String,
    /// The part of the world ("Europe", "North America").
    pub region: String,
    /// The carrier frequencies, Hz.
    pub carriers_hz: Vec<f64>,
}

impl WefaxStation {
    /// The upper-sideband dials of the carriers, Hz.
    pub fn dials_hz(&self) -> Vec<f64> {
        self.carriers_hz
            .iter()
            .map(|f| f - WEFAX_DIAL_OFFSET_HZ)
            .collect()
    }
}

/// One broadcast time of a radiofax station.
#[derive(Clone, Debug, PartialEq)]
pub struct WefaxBroadcast {
    /// The id of the station.
    pub station: String,
    /// The start, seconds after 00:00 UTC.
    pub utc_s: u32,
    /// Lines per minute.
    pub lpm: u32,
    /// The index of cooperation.
    pub ioc: u32,
    /// The product ("Surface analysis").
    pub product: String,
    /// The time is known only roughly.
    pub approximate: bool,
}

impl WefaxBroadcast {
    /// The settings of the stack `marine` (submode `wefax`) for this broadcast: lines per minute and index of
    /// cooperation, as choice keys.
    pub fn settings(&self) -> Vec<(String, Value)> {
        vec![
            ("fax_lpm".to_owned(), Value::Text(self.lpm.to_string())),
            ("fax_ioc".to_owned(), Value::Text(self.ioc.to_string())),
        ]
    }
}

/// A format error of `t` at `line`.
fn format_error(t: &Table, line: usize, reason: impl std::fmt::Display) -> DataError {
    DataError::Format {
        path: t.path.clone(),
        reason: format!("line {line}: {reason}"),
    }
}

/// The parts of a `|` list, without empty parts.
fn list(text: &str) -> impl Iterator<Item = &str> {
    text.split('|').map(str::trim).filter(|s| !s.is_empty())
}

/// `hh:mm` as seconds after midnight.
fn time_of_day(text: &str) -> Option<u32> {
    let (h, m) = text.split_once(':')?;
    let (h, m): (u32, u32) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    (h < 24 && m < 60).then_some(h * 3600 + m * 60)
}

/// The marine VHF channel plans of the file `path` (the format of `marine/vhf_channels.csv`), in the order of the file.
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_plans(path: &Path) -> Result<Vec<MarinePlan>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "plan",
            "plan_name",
            "label",
            "number",
            "suffix",
            "ship_mhz",
            "coast_mhz",
            "use",
            "flags",
            "power_w",
            "name",
        ],
    )?;
    let mut plans: Vec<MarinePlan> = Vec::new();
    for (line, r) in &t.rows {
        let id = t.text(r, "plan");
        if id.is_empty() {
            return Err(format_error(&t, *line, "the column `plan` is empty"));
        }
        let channel = MarineChannel {
            label: t.text(r, "label").to_owned(),
            number: u32::try_from(t.count(*line, r, "number")?)
                .map_err(|_| format_error(&t, *line, "number"))?,
            suffix: t.text(r, "suffix").to_owned(),
            ship_hz: (t.required(*line, r, "ship_mhz")? * 1e6).round(),
            coast_hz: (t.required(*line, r, "coast_mhz")? * 1e6).round(),
            use_: t.text(r, "use").to_owned(),
            flags: list(t.text(r, "flags")).map(str::to_owned).collect(),
            power_w: t.required(*line, r, "power_w")?,
            name: t.text(r, "name").to_owned(),
        };
        if channel.label.is_empty() {
            return Err(format_error(&t, *line, "the column `label` is empty"));
        }
        match plans.iter_mut().find(|p| p.id == id) {
            Some(p) => p.channels.push(channel),
            None => plans.push(MarinePlan {
                id: id.to_owned(),
                name: t.text(r, "plan_name").to_owned(),
                channels: vec![channel],
            }),
        }
    }
    Ok(plans)
}

/// The radiofax stations of the file `path` (the format of `hfdata/wefax_stations.csv`).
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_wefax_stations(path: &Path) -> Result<Vec<WefaxStation>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &[
            "id",
            "name",
            "callsigns",
            "country",
            "region",
            "carrier_khz",
        ],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let carriers_hz = list(t.text(r, "carrier_khz"))
            .map(|f| match f.parse::<f64>() {
                Ok(v) if v.is_finite() && v > 0.0 => Ok(v * 1e3),
                _ => Err(format_error(
                    &t,
                    *line,
                    format_args!("`{f}` is not a carrier frequency"),
                )),
            })
            .collect::<Result<Vec<f64>, DataError>>()?;
        out.push(WefaxStation {
            id: t.text(r, "id").to_owned(),
            name: t.text(r, "name").to_owned(),
            callsigns: list(t.text(r, "callsigns")).map(str::to_owned).collect(),
            country: t.text(r, "country").to_owned(),
            region: t.text(r, "region").to_owned(),
            carriers_hz,
        });
    }
    Ok(out)
}

/// The radiofax broadcast times of the file `path` (the format of `hfdata/wefax_broadcasts.csv`).
///
/// # Errors
///
/// [`DataError::Missing`], [`DataError::Read`], and [`DataError::Format`] for a row that does not have its format.
pub fn read_wefax_broadcasts(path: &Path) -> Result<Vec<WefaxBroadcast>, DataError> {
    let t = Table::read(
        path.to_owned(),
        &["station", "utc", "lpm", "ioc", "product", "approximate"],
    )?;
    let mut out = Vec::new();
    for (line, r) in &t.rows {
        let utc = t.text(r, "utc");
        let utc_s = time_of_day(utc)
            .ok_or_else(|| format_error(&t, *line, format_args!("`{utc}` is not hh:mm")))?;
        let count = |name: &str| -> Result<u32, DataError> {
            u32::try_from(t.count(*line, r, name)?).map_err(|_| format_error(&t, *line, name))
        };
        out.push(WefaxBroadcast {
            station: t.text(r, "station").to_owned(),
            utc_s,
            lpm: count("lpm")?,
            ioc: count("ioc")?,
            product: t.text(r, "product").to_owned(),
            approximate: match t.text(r, "approximate") {
                "true" => true,
                "false" | "" => false,
                v => {
                    return Err(format_error(
                        &t,
                        *line,
                        format_args!("`{v}` is not true or false"),
                    ))
                }
            },
        });
    }
    Ok(out)
}

impl Data {
    /// The marine VHF channel plans of `marine/vhf_channels.csv`, in the order of the file.
    ///
    /// # Errors
    ///
    /// As [`read_plans`].
    pub fn marine_plans(&self) -> Result<Vec<MarinePlan>, DataError> {
        read_plans(&self.folder().join(VHF_CHANNELS))
    }

    /// The radiofax stations of `hfdata/wefax_stations.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_wefax_stations`].
    pub fn wefax_stations(&self) -> Result<Vec<WefaxStation>, DataError> {
        read_wefax_stations(&self.folder().join(WEFAX_STATIONS))
    }

    /// The radiofax broadcast times of `hfdata/wefax_broadcasts.csv`.
    ///
    /// # Errors
    ///
    /// As [`read_wefax_broadcasts`].
    pub fn wefax_broadcasts(&self) -> Result<Vec<WefaxBroadcast>, DataError> {
        read_wefax_broadcasts(&self.folder().join(WEFAX_BROADCASTS))
    }

    /// The id of the marine VHF channel plan of the country `code`: the row of [`VHF_PLAN_COUNTRIES`] that lists it, else
    /// [`DEFAULT_VHF_PLAN`].
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], [`DataError::Format`] for a file without the columns `plan` and `countries`.
    pub fn marine_plan_of(&self, code: &str) -> Result<String, DataError> {
        let t = match Table::read(self.folder().join(VHF_PLAN_COUNTRIES), &["plan", "countries"]) {
            Ok(t) => t,
            Err(DataError::Missing { .. }) => return Ok(DEFAULT_VHF_PLAN.to_owned()),
            Err(e) => return Err(e),
        };
        Ok(t.rows
            .iter()
            .find(|(_, r)| t.text(r, "countries").split_whitespace().any(|c| c.eq_ignore_ascii_case(code)))
            .map_or_else(|| DEFAULT_VHF_PLAN.to_owned(), |(_, r)| t.text(r, "plan").to_owned()))
    }

    /// The marine VHF channel plan `id` ("INT", "EUR", "BAL", "USA", "CAN" in the repository's file), `None` when the
    /// file has no such plan.
    ///
    /// # Errors
    ///
    /// As [`Data::marine_plans`].
    pub fn marine_plan(&self, id: &str) -> Result<Option<MarinePlan>, DataError> {
        Ok(self
            .marine_plans()?
            .into_iter()
            .find(|p| p.id.eq_ignore_ascii_case(id)))
    }

    /// The radiofax station `id`, `None` when the file has no such station.
    ///
    /// # Errors
    ///
    /// As [`Data::wefax_stations`].
    pub fn wefax_station(&self, id: &str) -> Result<Option<WefaxStation>, DataError> {
        Ok(self
            .wefax_stations()?
            .into_iter()
            .find(|s| s.id.eq_ignore_ascii_case(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// The files of the repository's data folder, read one by one so that another file of the folder does not stop
    /// these tests.
    struct Files(PathBuf);

    impl Files {
        fn marine_plans(&self) -> Result<Vec<MarinePlan>, DataError> {
            read_plans(&self.0.join(VHF_CHANNELS))
        }
        fn marine_plan(&self, id: &str) -> Result<Option<MarinePlan>, DataError> {
            Ok(self
                .marine_plans()?
                .into_iter()
                .find(|p| p.id.eq_ignore_ascii_case(id)))
        }
        fn wefax_stations(&self) -> Result<Vec<WefaxStation>, DataError> {
            read_wefax_stations(&self.0.join(WEFAX_STATIONS))
        }
        fn wefax_station(&self, id: &str) -> Result<Option<WefaxStation>, DataError> {
            Ok(self.wefax_stations()?.into_iter().find(|s| s.id == id))
        }
        fn wefax_broadcasts(&self) -> Result<Vec<WefaxBroadcast>, DataError> {
            read_wefax_broadcasts(&self.0.join(WEFAX_BROADCASTS))
        }
    }

    fn data() -> Files {
        Files(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data"))
    }

    /// The numbering rule of ITU RR Appendix 18: the ship frequency of a channel, MHz.
    fn rule_ship_mhz(n: u32) -> Option<f64> {
        match n {
            1..=28 => Some(156.050 + 0.050 * f64::from(n - 1)),
            60..=88 => Some(156.025 + 0.050 * f64::from(n - 60)),
            _ => None,
        }
    }

    #[test]
    fn every_row_of_the_plans_loads() {
        let d = data();
        let plans = d.marine_plans().unwrap();
        let counts: Vec<(&str, usize)> = plans
            .iter()
            .map(|p| (p.id.as_str(), p.channels.len()))
            .collect();
        assert_eq!(
            counts,
            vec![
                ("INT", 59),
                ("EUR", 60),
                ("BAL", 59),
                ("USA", 54),
                ("CAN", 58)
            ]
        );
        assert_eq!(counts.iter().map(|c| c.1).sum::<usize>(), 290);
        for p in &plans {
            // The setting text has one entry per channel and only frequencies of the file.
            let text = p.plan_text();
            assert_eq!(text.split(';').count(), p.channels.len(), "{}", p.id);
            assert_eq!(
                text.matches(":data").count(),
                p.channels.iter().filter(|c| !c.voice()).count()
            );
            assert_eq!(p.settings(), vec![("plan".to_owned(), Value::Text(text))]);
        }
        assert_eq!(d.marine_plan("usa").unwrap().unwrap().id, "USA");
        assert!(d.marine_plan("XYZ").unwrap().is_none());
    }

    #[test]
    fn plan_facts() {
        let d = data();
        for (n, f) in [
            (1, 156.050),
            (16, 156.800),
            (28, 157.400),
            (60, 156.025),
            (70, 156.525),
            (88, 157.425),
        ] {
            assert!((rule_ship_mhz(n).unwrap() - f).abs() < 1e-9);
        }
        assert!(rule_ship_mhz(29).is_none() && rule_ship_mhz(89).is_none());
        let int = d.marine_plan("INT").unwrap().unwrap();
        for c in &int.channels {
            if c.suffix.is_empty() {
                assert!(
                    (rule_ship_mhz(c.number).unwrap() * 1e6 - c.ship_hz).abs() < 1.0,
                    "{}",
                    c.label
                );
                if !c.simplex() {
                    assert!((c.coast_hz - c.ship_hz - 4.6e6).abs() < 1.0, "{}", c.label);
                }
            }
        }
        for code in ["INT", "EUR", "BAL", "USA", "CAN"] {
            let r = d.marine_plan(code).unwrap().unwrap();
            for l in ["87A", "87B", "88A", "88B"] {
                assert!(r.by_label(l).is_some(), "{code} {l}");
            }
            assert!((r.by_label("87B").unwrap().ship_hz - 161.975e6).abs() < 1.0);
        }
        assert_eq!(
            int.by_label("ch 16").unwrap().title(),
            "16 Distress, safety, calling"
        );
        assert_eq!(int.by_label("Ch.16").unwrap().label, "16");
        assert_eq!(int.by_label("VHF 87B").unwrap().label, "87B");
        let usa = d.marine_plan("USA").unwrap().unwrap();
        assert!((usa.by_label("22a").unwrap().ship_hz - 157.100e6).abs() < 1.0);
        assert_eq!(usa.by_label("22").unwrap().label, "22A");
        let eur = d.marine_plan("EUR").unwrap().unwrap();
        assert_eq!(eur.by_label("37").map(|c| c.ship_hz), Some(157.850e6));
        assert_eq!(int.by_label("70").unwrap().use_, "dsc");
        let (c, coast) = int.by_frequency(160.65e6).unwrap();
        assert_eq!((c.label.as_str(), coast), ("1", true));
        assert_eq!(
            int.from_dsc_text("VHF ch 1016 (ship simplex)")
                .unwrap()
                .label,
            "16"
        );
        assert_eq!(int.from_dsc_text("ch 06").unwrap().label, "6");
        assert_eq!(
            int.step_voice("16", true).map(|c| c.label.clone()),
            int.voice_channels()
                .iter()
                .skip_while(|c| c.label != "16")
                .nth(1)
                .map(|c| c.label.clone())
        );
        let bal = d.marine_plan("BAL").unwrap().unwrap();
        assert_eq!(bal.by_number(61, "").unwrap().name, "Tallinn");
        let v = int.voice_frequencies();
        assert!(v.windows(2).all(|w| w[0] < w[1]));
        assert!(int.all_frequencies().len() > v.len());
    }

    #[test]
    fn every_row_of_the_radiofax_files_loads() {
        let d = data();
        let stations = d.wefax_stations().unwrap();
        let broadcasts = d.wefax_broadcasts().unwrap();
        assert_eq!((stations.len(), broadcasts.len()), (28, 50));
        for b in &broadcasts {
            assert!(stations.iter().any(|s| s.id == b.station), "{}", b.station);
        }
        for s in &stations {
            assert!(
                !s.carriers_hz.is_empty() && !s.callsigns.is_empty(),
                "{}",
                s.id
            );
        }
        let ddk = d.wefax_station("DDK").unwrap().unwrap();
        assert_eq!(ddk.callsigns, ["DDH3", "DDK3", "DDK6"]);
        assert_eq!(ddk.dials_hz(), vec![3_853_100.0, 7_878_100.0, 13_880_600.0]);
        let first = &broadcasts[0];
        assert_eq!(
            (first.station.as_str(), first.utc_s, first.lpm, first.ioc),
            ("DDK", 4 * 3600 + 30 * 60, 120, 576)
        );
        assert!(first.approximate);
        assert_eq!(
            first.settings(),
            vec![
                ("fax_lpm".to_owned(), Value::Text("120".into())),
                ("fax_ioc".to_owned(), Value::Text("576".into())),
            ]
        );
    }
}
