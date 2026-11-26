//! The GSM cell table: cells keyed by BSIC and channel, merged from each capture, and the carriers of the newest capture.

use super::search::{Carrier, GsmFound, GsmSearchOut};
use super::si::{self, Lai, Si, Si3Fields};
use super::GsmSearch;
use crate::bands::{arfcn_to_hz, arfcn_to_hz_ext, hz_to_arfcn, hz_to_arfcn_ext, railway_band};
use crate::decoder::{Capture, Log};
use crate::typed::GsmrCellRow;
use sdrkit_core::interface::{Hertz, Time};
use sdrkit_util_ids::mcc::{self, Plmn};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{RowOut, Shaped};
use sdrkit_util_objects::{Format, ObjectTable, Seen, Shift};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The most cells the table keeps.
pub const CELL_CAP: usize = 200;
/// The most carriers the Carriers view lists.
pub const CARRIER_CAP: usize = 24;
/// The channel raster, Hz (45.005 2).
const RASTER_HZ: f64 = 200e3;

sdrkit_core::choice_enum! {
    /// The band of an ARFCN.
    pub enum GsmBand {
        /// The railway band (ARFCN 955 to 973).
        GsmR = ("gsm_r", "GSM-R"),
        /// GSM 900.
        Gsm900 = ("gsm_900", "GSM 900"),
        /// GSM 850.
        Gsm850 = ("gsm_850", "GSM 850"),
        /// DCS 1800.
        Dcs1800 = ("dcs_1800", "DCS 1800"),
    }
}
/// The channel of a cell in its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// The ARFCN, when the radio centre is known.
    Arfcn(u16),
    /// Without the radio centre: the carrier offset from 0 Hz in steps of the 200 kHz raster.
    Raster(i32),
}

/// The key of a cell: its BSIC and its channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GsmKey {
    /// BSIC.
    pub bsic: u8,
    /// The channel.
    pub channel: Channel,
}

/// "960/1-2" for ARFCN 960 and BSIC 1-2, "+400kHz/1-2" without the radio centre.
impl fmt::Display for GsmKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (ncc, bcc) = (self.bsic >> 3, self.bsic & 7);
        match self.channel {
            Channel::Arfcn(a) => write!(f, "{a}/{ncc}-{bcc}"),
            Channel::Raster(s) => write!(f, "{:+}kHz/{ncc}-{bcc}", s * 200),
        }
    }
}

impl sdrkit_graph::Item for GsmCell {
    const KIND: &'static str = "cell.gsm_cell";
}

/// The location area of a cell as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LaiOut {
    /// "MCC-MNC".
    pub plmn: String,
    /// Operator name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    /// LAC.
    pub lac: u16,
}

/// A GSM cell of the table. When it was seen is the row's [`Seen`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GsmCell {
    /// ARFCN, -1 without the radio centre.
    pub arfcn: i64,
    /// The nominal frequency of the ARFCN, Hz (the carrier offset from 0 Hz without the radio centre).
    pub frequency_hz: f64,
    /// In the railway band.
    pub railway_band: bool,
    /// BSIC.
    pub bsic: u8,
    /// NCC.
    pub ncc: u8,
    /// BCC.
    pub bcc: u8,
    /// Frame number of the newest SCH.
    #[serde(rename = "fn")]
    pub fnum: u32,
    /// Carrier offset (actual - nominal), Hz.
    pub carrier_offset_hz: f64,
    /// LAI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lai: Option<LaiOut>,
    /// SI3 fields, with the newest selection and RACH parameters of SI1, SI2 and SI4.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si3: Option<Si3Fields>,
    /// SI1 cell allocation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si1_cell_allocation: Option<Vec<u16>>,
    /// SI2 BCCH neighbours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si2_neighbours: Option<Vec<u16>>,
    /// NCC permitted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ncc_permitted: Option<u8>,
    /// The System Information type codes seen, in order of code.
    pub type_codes: Vec<u8>,
    /// SI messages decoded.
    pub si_messages: u64,
    /// FIRE failures.
    pub si_failed: u64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR from the SCH, dB.
    pub snr_db: f64,
}

impl GsmCell {
    /// The actual frequency of the carrier, Hz (from 0 Hz without the radio centre).
    pub fn actual_hz(&self) -> f64 {
        self.frequency_hz + self.carrier_offset_hz
    }

    /// The names of the types seen, in order of code.
    pub fn types_seen(&self) -> Vec<String> {
        self.type_codes.iter().map(|&c| si::type_name(c)).collect()
    }

    /// The band of the ARFCN (TS 45.005 clause 2): the railway band, else the GSM band.
    pub fn band(&self) -> Option<GsmBand> {
        let a = u32::try_from(self.arfcn).ok()?;
        Some(match a {
            955..=973 => GsmBand::GsmR,
            0..=124 | 974..=1023 => GsmBand::Gsm900,
            128..=251 => GsmBand::Gsm850,
            512..=885 => GsmBand::Dcs1800,
            _ => return None,
        })
    }

    fn label(&self) -> String {
        if self.arfcn >= 0 {
            format!("ARFCN {}", self.arfcn)
        } else {
            format!("carrier at {:+.1} kHz", self.actual_hz() / 1e3)
        }
    }
}

impl Shift for GsmCell {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl GsmCell {
    /// The cell as a row of the survey.
    pub fn survey(&self, key: &str) -> crate::scan::SurveyCell {
        let lai = self.lai.as_ref();
        crate::scan::SurveyCell {
            key: key.to_string(),
            generation: crate::scan::Generation::Second,
            plmn: lai.map(|l| l.plmn.clone()),
            operator_name: lai.and_then(|l| l.operator.clone()),
            band: self.band().map(|b| <GsmBand as sdrkit_core::Choice>::CHOICES[usize::from(sdrkit_core::Choice::index(b))].label.to_string()),
            channel: (self.arfcn >= 0).then_some(self.arfcn),
            frequency_hz: self.frequency_hz,
            id_word: "BSIC",
            pci: i64::from(self.bsic),
            cell_identity: self.si3.as_ref().map(|s| i64::from(s.cell_id)),
            area_code: lai.map(|l| i64::from(l.lac)),
            level_dbfs: Some(self.level_dbfs),
        }
    }
}

impl Shaped for GsmCell {
    type Row = GsmrCellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let lai = self.lai.as_ref();
        out.put(&GsmrCellRow {
            plmn: lai.map(|l| l.plmn.as_str()),
            operator: lai.and_then(|l| l.operator.as_deref()),
            arfcn: (self.arfcn >= 0).then_some(self.arfcn),
            frequency: Hertz(self.frequency_hz),
            bsic: i64::from(self.bsic),
            ncc: i64::from(self.ncc),
            bcc: i64::from(self.bcc),
            lac: lai.map(|l| i64::from(l.lac)),
            cell_id: self.si3.as_ref().map(|s| i64::from(s.cell_id)),
            band: self.band(),
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            last_heard: columns::last_heard(seen).map(Time),
            generation: crate::scan::Generation::Second,
            name: &self.survey("").label(),
        });
    }
}

/// A carrier of the newest capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarrierRow {
    /// Frequency, Hz (the offset from 0 Hz without the radio centre).
    pub frequency_hz: f64,
    /// dB over the floor.
    pub level_db: f64,
    /// An SCH decoded on it.
    pub synchronised: bool,
}

/// What the stack keeps: the cells, and the carriers of the newest capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GsmTables {
    /// The cells, keyed by BSIC and channel.
    pub cells: ObjectTable<GsmKey, GsmCell>,
    /// The carriers of the newest capture, strongest first.
    pub carriers: Vec<CarrierRow>,
}

impl Default for GsmTables {
    fn default() -> Self {
        GsmTables {
            cells: ObjectTable::new(CELL_CAP),
            carriers: Vec::new(),
        }
    }
}

impl Shift for GsmTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for GsmTables {
    const FORMAT: Format = Format::new("cell.gsmr_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
        self.carriers.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
        self.carriers = saved.carriers;
        self.carriers.truncate(CARRIER_CAP);
    }
}

impl GsmTables {
    /// Merges what a capture held. `ext_band` reads ARFCNs on the 850 and 1800 MHz rasters too.
    pub fn merge(
        &mut self,
        cap: &Capture,
        out: GsmSearchOut,
        ext_band: bool,
        log: &mut Log<GsmSearch>,
    ) {
        let known = cap.centre_known();
        let base = if known { cap.centre_hz } else { 0.0 };
        self.carriers = out
            .carriers
            .iter()
            .map(|(c, s): &(Carrier, bool)| CarrierRow {
                frequency_hz: base + c.offset_hz,
                level_db: c.level_db,
                synchronised: *s,
            })
            .collect();
        for f in out.cells {
            self.merge_cell(cap, f, known, ext_band, log);
        }
    }

    fn merge_cell(
        &mut self,
        cap: &Capture,
        f: GsmFound,
        known: bool,
        ext_band: bool,
        log: &mut Log<GsmSearch>,
    ) {
        let t = cap.end_time;
        let actual = if known { cap.centre_hz } else { 0.0 } + f.offset_hz;
        let arfcn = if !known {
            None
        } else if ext_band {
            hz_to_arfcn_ext(actual)
        } else {
            hz_to_arfcn(actual)
        };
        let nominal = arfcn
            .and_then(|a| {
                if ext_band {
                    arfcn_to_hz_ext(a)
                } else {
                    arfcn_to_hz(a)
                }
            })
            .unwrap_or(actual);
        let channel = match arfcn.and_then(|a| u16::try_from(a).ok()) {
            Some(a) => Channel::Arfcn(a),
            None => Channel::Raster((actual / RASTER_HZ).round() as i32),
        };
        let key = GsmKey {
            bsic: f.bsic,
            channel,
        };
        let Ok(up) = self.cells.upsert(&key, log.stamp(t), || GsmCell {
            arfcn: arfcn.map_or(-1, i64::from),
            frequency_hz: nominal,
            railway_band: arfcn.is_some_and(railway_band),
            bsic: f.bsic,
            ncc: f.bsic >> 3,
            bcc: f.bsic & 7,
            ..GsmCell::default()
        }) else {
            return;
        };
        let cell = up.row;
        if up.new {
            let text = if cell.arfcn >= 0 {
                format!(
                    "ARFCN {} at {:.1} MHz{}, BSIC {}-{}",
                    cell.arfcn,
                    nominal / 1e6,
                    if cell.railway_band {
                        " (GSM-R band)"
                    } else {
                        ""
                    },
                    cell.ncc,
                    cell.bcc
                )
            } else {
                format!(
                    "carrier at {:+.1} kHz, BSIC {}-{}",
                    actual / 1e3,
                    cell.ncc,
                    cell.bcc
                )
            };
            log.event(t, key.to_string(), text);
        }
        cell.fnum = f.fnum;
        cell.level_dbfs = f.level_dbfs;
        cell.snr_db = f.snr_db;
        cell.carrier_offset_hz = actual - nominal;
        cell.si_failed += u64::from(f.failed);
        *log.failed += u64::from(f.failed);
        for (_, m) in &f.frames {
            let Some(s) = si::parse(m) else { continue };
            *log.decoded += 1;
            cell.si_messages += 1;
            let code = s.code();
            if let Err(at) = cell.type_codes.binary_search(&code) {
                cell.type_codes.insert(at, code);
            }
            apply(cell, &key, s, t, log);
        }
    }
}

/// The LAI as published, with the operator name.
fn lai_out(lai: &Lai) -> LaiOut {
    let plmn = Plmn {
        mcc: lai.mcc,
        mnc: lai.mnc,
        mnc_digits: lai.mnc_digits,
    }
    .to_string();
    let operator = Plmn::parse(&plmn).and_then(mcc::operator).map(String::from);
    LaiOut {
        plmn,
        operator,
        lac: lai.lac,
    }
}

/// "MCC-MNC Operator, LAC L" of a LAI.
fn lai_text(l: &LaiOut) -> String {
    match &l.operator {
        Some(op) => format!("{} {op}, LAC {}", l.plmn, l.lac),
        None => format!("{}, LAC {}", l.plmn, l.lac),
    }
}

/// Sets the LAI of `cell`. If the LAI is new or its LAC or network changed, and it does not come from an SI3, adds a log line.
fn set_lai(
    cell: &mut GsmCell,
    key: &GsmKey,
    lai: &Lai,
    from_si3: bool,
    t: f64,
    log: &mut Log<GsmSearch>,
) {
    let new = lai_out(lai);
    let changed = cell
        .lai
        .as_ref()
        .is_none_or(|l| l.lac != new.lac || l.plmn != new.plmn);
    if changed && !from_si3 {
        log.event(
            t,
            key.to_string(),
            format!("{}: {}", cell.label(), lai_text(&new)),
        );
    }
    cell.lai = Some(new);
}

/// Applies one System Information message to `cell`, the cell with key `key` (TS 44.018 9.1.31 to 9.1.36).
fn apply(cell: &mut GsmCell, key: &GsmKey, s: Si, t: f64, log: &mut Log<GsmSearch>) {
    match s {
        Si::Si1 {
            cell_allocation,
            rach,
        } => {
            if let Some(f) = cell.si3.as_mut() {
                si::apply_rach(f, &rach);
            }
            if cell_allocation.is_some() {
                cell.si1_cell_allocation = cell_allocation;
            }
        }
        Si::Si2 {
            neighbours,
            ncc_permitted,
            rach,
        } => {
            if let Some(f) = cell.si3.as_mut() {
                si::apply_rach(f, &rach);
            }
            if neighbours.is_some() {
                cell.si2_neighbours = neighbours;
                cell.ncc_permitted = Some(ncc_permitted);
            }
        }
        Si::Si3 { lai, fields } => {
            let new_cell = cell
                .si3
                .as_ref()
                .is_none_or(|x| x.cell_id != fields.cell_id);
            let new_lac = cell.lai.as_ref().is_none_or(|l| l.lac != lai.lac);
            set_lai(cell, key, &lai, true, t, log);
            let id = fields.cell_id;
            cell.si3 = Some(fields);
            if new_cell || new_lac {
                let l = cell.lai.as_ref().map(lai_text).unwrap_or_default();
                log.event(
                    t,
                    key.to_string(),
                    format!("{}: {l}, cell {id}", cell.label()),
                );
            }
        }
        Si::Si4 { lai, sel, rach } => {
            set_lai(cell, key, &lai, false, t, log);
            if let Some(f) = cell.si3.as_mut() {
                si::apply_selection(f, &sel);
                si::apply_rach(f, &rach);
            }
        }
        Si::Other(_) => {}
    }
}
