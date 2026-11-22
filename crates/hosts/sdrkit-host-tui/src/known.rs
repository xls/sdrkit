//! The region data of the application: the preset that a stack starts from, and the table of the known stations.

use crate::api::{Decl, Handle, Id, Presets, ResultsDecl, TableDecl, Type, Value};
use crate::results::{Results, Row, Rows};
use sdrkit_presets::{Data, DataError, Options, Preset, PresetError, Region, StationKind};
use std::path::Path;

/// The key of the table of the known stations.
pub const KNOWN_STATIONS: &str = "known_stations";

/// The open data folder, and the region that `--region` and `--city` name.
#[derive(Debug)]
pub struct Regional {
    /// The data folder.
    pub data: Data,
    /// The country and the city.
    pub region: Region,
    /// The same folder in the presets part of the library, which fits the settings of a preset to its stack.
    presets: Presets,
}

impl Regional {
    /// Opens the data folder `folder` for the country `region` and the city `city`.
    ///
    /// # Errors
    ///
    /// The sentence for the status line: that `region` is not a country code, "no region data loaded: <path>" if
    /// the folder does not exist, else why its plan files cannot be read.
    pub fn open(
        folder: &Path,
        region: &str,
        city: Option<&str>,
    ) -> Result<Regional, String> {
        match Data::open(folder) {
            Ok(data) => {
                let region = data.region(region).map_err(|list| {
                    format!("--region: the data has no plans for {region}; g lists the regions ({} of them)", list.lines().count())
                })?;
                Ok(Regional {
                    data,
                    region: region.with_city(city.unwrap_or("")),
                    presets: Presets::open(folder).map_err(|e| e.message)?,
                })
            }
            Err(DataError::Missing { path }) => {
                Err(format!("no region data loaded: {}", path.display()))
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// The preset of `stack` on the frequency `freq_hz` (else that of the region) in the submode `submode`. `Ok(None)`
    /// if the data has nothing for the stack in the region.
    ///
    /// # Errors
    ///
    /// [`PresetError::Data`] for a file that cannot be read.
    pub fn preset(
        &self,
        stack: &str,
        freq_hz: Option<f64>,
        submode: Option<&str>,
    ) -> Result<Option<Preset>, PresetError> {
        let options = Options {
            frequency_hz: freq_hz,
            submode: submode.map(String::from),
            settings: Vec::new(),
        };
        match self.data.preset(stack, &self.region, &options) {
            Ok(p) => Ok(Some(p)),
            Err(PresetError::NoData { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// The settings that the data has for `stack` in the region, on the frequency `freq_hz` (else that of the region) in
    /// the submode `submode`, as values of the settings of the stack type in `engine`, by setting id.
    ///
    /// # Errors
    ///
    /// A sentence that names the setting, if the stack has no such setting or the setting does not take the value.
    pub fn settings(
        &self,
        engine: &Handle,
        stack: &str,
        freq_hz: Option<f64>,
        submode: Option<&str>,
    ) -> Result<Vec<(Id, Value)>, String> {
        let region = self.region.of_stack(stack);
        self.presets
            .settings(engine, stack, region.country(), region.city(), freq_hz, submode)
            .map_err(|e| e.message)
    }

    /// The channels that the data has for `stack` in the region: the line that a list shows (`7  184.5 MHz  7 MHz`) and
    /// the frequency in Hz.
    ///
    /// # Errors
    ///
    /// A file that cannot be read, as a sentence.
    pub fn channels(&self, stack: &str) -> Result<Vec<(String, f64)>, String> {
        let channels = self
            .data
            .stack_channels(stack, &self.region)
            .map_err(|e| e.to_string())?;
        Ok(channels
            .into_iter()
            .map(|c| {
                let mut line = format!("{:<8}", c.label);
                crate::format::hz(&mut line, c.frequency_hz);
                if let Some(w) = c.width_hz {
                    line.push_str("  width ");
                    crate::format::hz(&mut line, w);
                }
                (line, c.frequency_hz)
            })
            .collect())
    }

    /// The number of channels that a scan of `stack` visits in the region.
    pub fn scan_channels(&self, stack: &str) -> usize {
        let ids = self.data.plan_ids(self.region.of_stack(stack).country());
        let ids: Vec<&str> = ids.iter().map(String::as_str).collect();
        self.data.scan_channels(&ids).get(stack).map_or(0, Vec::len)
    }

    /// The table of the known stations of the service of `stack` in the country, those of the city first. `None` for
    /// a stack whose service has no lists of stations. A country without a list has a table without rows.
    ///
    /// # Errors
    ///
    /// A list that cannot be read, as a sentence.
    pub fn known_stations(&self, stack: &str) -> Result<Option<Results>, String> {
        let Some(kind) = StationKind::of_stack(stack) else {
            return Ok(None);
        };
        let stations = match self.data.known_stations(kind, self.region.of_stack(stack)) {
            Ok(s) => s,
            Err(DataError::Missing { .. }) => Vec::new(),
            Err(e) => return Err(e.to_string()),
        };
        let column = |key: &str, label: &str, ty: Type, unit: &str| Decl {
            key: key.into(),
            label: label.into(),
            ty,
            unit: unit.into(),
            ..Decl::default()
        };
        let columns = vec![
            column("city", "City", Type::String, ""),
            column("freq", "Frequency", Type::Hz, "Hz"),
            column("width_hz", "Width", Type::Hz, "Hz"),
            column("standard", "Standard", Type::String, ""),
            column("network", "Network", Type::String, ""),
            column("services", "Services", Type::String, ""),
            column("snr_db", "SNR", Type::F64, "dB"),
            column("reports", "Reports", Type::I64, ""),
            column("last_seen", "Last seen", Type::String, ""),
        ];
        let rows = stations
            .iter()
            .enumerate()
            .map(|(n, s)| Row {
                key: format!("{} {} {}", n + 1, s.city, s.frequency_hz / 1e6),
                cells: vec![
                    Value::String(s.city.clone()),
                    Value::Hz(s.frequency_hz),
                    s.width_hz.map_or(Value::None, Value::Hz),
                    Value::String(s.standard.clone()),
                    Value::String(s.network.clone()),
                    Value::String(s.services.join(", ")),
                    s.snr_db.map_or(Value::None, Value::F64),
                    Value::I64(i64::from(s.reports)),
                    Value::String(s.last_seen.clone()),
                ],
            })
            .collect::<Vec<Row>>();
        let mut results = Results::default();
        results.decl = ResultsDecl {
            fields: Vec::new(),
            tables: vec![TableDecl {
                key: KNOWN_STATIONS.into(),
                label: "Known stations".into(),
                cap: rows.len(),
                columns,
                label_column: None,
            }],
        };
        results.tables = vec![Rows { rows }];
        Ok(Some(results))
    }
}

/// The countries that the data folder `folder` has plans for.
///
/// # Errors
///
/// A folder that does not open, as a sentence.
pub fn regions(folder: &std::path::Path) -> Result<Vec<sdrkit_presets::RegionChoice>, String> {
    match Data::open(folder) {
        Ok(data) => Ok(data.regions()),
        Err(DataError::Missing { path }) => {
            Err(format!("no region data loaded: {}", path.display()))
        }
        Err(e) => Err(e.to_string()),
    }
}
