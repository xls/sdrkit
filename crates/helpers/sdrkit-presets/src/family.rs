//! The settings that the family files of the data folder have for a stack in a country.
//!
//! A file without a country in its rows (`wsjt/dials.csv`, `general/presets.csv`, the FPV channels) gives its settings to
//! every country. A file whose rows have a code gives the row with the code of the country; `ism/wisun_bands.csv` gives
//! the first plan that lists the country. A file whose rows are named
//! by an area (`marine/vhf_channels.csv`, `cell/band_order.csv`, `avdata/datalink_channels.csv`) gives none: the host
//! offers its rows by name.

use crate::data::Data;
use crate::error::DataError;
use sdrkit_core::Value;

/// The settings of one stack, and the frequency to tune if the row has one.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct FamilySettings {
    pub frequency_hz: Option<f64>,
    pub settings: Vec<(String, Value)>,
}

/// The weak-signal stacks that take the dial list. Each takes the band edges too.
const WSJT_DIALS: &[&str] = &["wsjt", "ft8", "ft4", "ft2", "wspr"];
/// The weak-signal stacks that take the band edges only.
const WSJT_EDGES: &[&str] = &[
    "jt65", "jt9", "jt4", "q65", "fst4", "fst4w", "msk144", "js8",
];

/// `r`, with a file that the folder lacks as `None`.
fn present<T>(r: Result<T, DataError>) -> Result<Option<T>, DataError> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(DataError::Missing { .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

impl Data {
    /// The settings that the family files have for `stack` in the country `code` (upper case). Empty for a stack
    /// without such a file, a country without a row, or a folder without the file.
    pub(crate) fn family_settings(
        &self,
        stack: &str,
        code: &str,
    ) -> Result<FamilySettings, DataError> {
        let mut out = FamilySettings::default();
        match stack {
            s if WSJT_DIALS.contains(&s) || WSJT_EDGES.contains(&s) => {
                let files = present(self.wsjt_dials())?.zip(present(self.wsjt_bands())?);
                if let Some((dials, bands)) = files {
                    out.settings = crate::wsjt::settings(&dials, &bands);
                    if WSJT_EDGES.contains(&s) {
                        out.settings.retain(|(k, _)| k != "dials");
                    }
                }
            }
            "general" => {
                if let Some(p) = present(self.general_presets())? {
                    out.settings.push(crate::general::presets_setting(&p));
                }
            }
            "atv" => {
                out.settings = self
                    .atv_plan_of_country(code)
                    .map(|p| p.settings())
                    .unwrap_or_default();
            }
            "fpv" => {
                out.settings = self
                    .fpv_channels()
                    .map(|p| p.settings())
                    .unwrap_or_default();
            }
            "halow" => {
                if let Some(rows) = present(self.halow_regions())? {
                    if let Some(r) = crate::wlan::halow_region_of(&rows, code) {
                        out.frequency_hz = r.first_channel_hz();
                        out.settings = r.settings();
                    }
                }
            }
            "wifi" => {
                if let Some(rows) = present(self.wifi_domains())? {
                    let domain = self.wifi_domain_of(code);
                    out.settings =
                        crate::wlan::wifi_settings(&rows, &domain, false).unwrap_or_default();
                }
            }
            "zwave" => {
                if let Some(rows) = present(self.zwave_regions())? {
                    if let Some(r) = crate::ism::zwave_region_of(&rows, code) {
                        out.frequency_hz = Some(r.centre_hz);
                        out.settings = r.settings();
                    }
                }
            }
            "wisun" => {
                if let Some(plans) = present(self.wisun_bands())? {
                    if let Some((hz, s)) = crate::ism::wisun_country_settings(&plans, code) {
                        out.frequency_hz = Some(hz);
                        out.settings = s;
                    }
                }
            }
            _ => {}
        }
        Ok(out)
    }
}
