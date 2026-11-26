//! The LTE cells stack (`cell`): PSS and SSS, MIB, and SIB1 to SIB8 and SIB24 on SI-RNTI. Paging, random access responses, every
//! UE-specific RNTI, the uplink and user data are never decoded.
//!
//! Standard: 3GPP TS 36.211, TS 36.212, TS 36.213 and TS 36.331.

pub mod cancel;
pub mod gen;
pub mod irat;
pub mod msg;
pub mod phy;
pub mod search;

use crate::bands::{apply_mfbi, band_choice, earfcn_to_mhz, lte_band, nearest_earfcn, BandChoice};
use crate::decoder::{
    build_search, capture_setting, plural, BandList, Capture, FoundCarrier, Headline, Log, RowSync, SearchConfig,
    SearchStack, BAND_ORDER, BAND_SEARCH, SEARCH_BANDS, SEARCH_DWELL,
};
use crate::typed::{
    CellBandRow, CellCarrierRow, CellDetailRow, CellFields, CellInterCarrierRow, CellNeighbourRow, CellOperatorRow,
    CellOtherCarrierRow,
    CellPlmnRow, CellRow, CellSibRow, CellSiteRow, CELL_BANDS, CELL_CARRIERS, CELL_CELLS, CELL_DETAILS,
    CELL_INTER_CARRIERS, CELL_LTE_CELL, CELL_OTHER_CARRIERS, CELL_NEIGHBOURS, CELL_OPERATORS, CELL_PLMNS, CELL_SIBS, CELL_SITES,
};
use msg::{Bcch, SibEntry};
use sdrkit_core::interface::{EventKind, Hertz, Seconds, Time};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, GenConfig, Generator, Receiver, Schema, Setting, SettingError,
    SettingId, StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_ids::mcc::{self, Plmn};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift};
use search::{LteSearchOut, LteSearcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "cell";
/// The lowest input rate, Hz: the central 72 subcarriers at 1.92 Msps.
pub const MIN_RATE_HZ: f64 = 1.92e6;

/// The notice the stack publishes.
pub const BANNER_TEXT: &str = "Receive only: the public information every LTE cell broadcasts to all phones (operator, band, \
cell identity). Nothing addressed to a phone is decoded.";

sdrkit_core::choice_enum! {
    /// The duplex of a cell.
    #[derive(Default, Serialize, Deserialize)]
    pub enum Duplex {
        /// Frequency division.
        #[default]
        #[serde(rename = "FDD")]
        Fdd = ("FDD", "FDD"),
        /// Time division.
        #[serde(rename = "TDD")]
        Tdd = ("TDD", "TDD"),
    }
}

sdrkit_core::choice_enum! {
    /// The length of a cyclic prefix or of a PHICH.
    #[derive(Default, Serialize, Deserialize)]
    pub enum Length {
        /// Normal.
        #[default]
        #[serde(rename = "normal")]
        Normal = ("normal", "Normal"),
        /// Extended.
        #[serde(rename = "extended")]
        Extended = ("extended", "Extended"),
    }
}

/// The MIB as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MibOut {
    /// RBs.
    pub n_rb: usize,
    /// Bandwidth, MHz.
    pub bandwidth_mhz: f64,
    /// CRS ports.
    pub ports: usize,
    /// The PHICH duration.
    pub phich_duration: Length,
    /// Ng index (1/6, 1/2, 1, 2).
    pub phich_ng_index: u8,
    /// SFN of the first whole frame.
    pub sfn_first_whole_frame: u32,
}

/// One scheduling entry.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SchedOut {
    /// Period, frames.
    pub period_frames: u16,
    /// SIB numbers.
    pub sibs: Vec<u16>,
}

/// SIB1 as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1Out {
    /// PLMNs.
    pub plmns: Vec<String>,
    /// Operator of the primary PLMN.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    /// TAC.
    pub tac: u16,
    /// Cell identity.
    pub cell_id: u32,
    /// eNB.
    pub enb: u32,
    /// Sector.
    pub sector: u32,
    /// Band.
    pub band: u16,
    /// Barred.
    pub barred: bool,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: i32,
    /// p-Max, dBm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_max_dbm: Option<i32>,
    /// SI window, ms.
    pub si_window_ms: u16,
    /// Value tag.
    pub value_tag: u8,
    /// Scheduling list.
    pub scheduling: Vec<SchedOut>,
    /// Primary MCC.
    #[serde(skip)]
    pub mcc: u16,
    /// Primary MNC.
    #[serde(skip)]
    pub mnc: u16,
}

/// The time alignment timer: milliseconds, or `None` for "infinity" (TS 36.331 TimeAlignmentTimer).
mod time_alignment {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    const INFINITY: &str = "infinity";

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum Form<'a> {
        Ms(u16),
        Text(&'a str),
    }

    pub fn serialize<S: Serializer>(v: &Option<u16>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(ms) => Form::Ms(*ms),
            None => Form::Text(INFINITY),
        }
        .serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u16>, D::Error> {
        match Form::deserialize(d)? {
            Form::Ms(ms) => Ok(Some(ms)),
            Form::Text(INFINITY) => Ok(None),
            Form::Text(_) => Err(serde::de::Error::custom("not a time alignment timer")),
        }
    }
}

/// SIB2 as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib2Out {
    /// referenceSignalPower, dBm.
    pub ref_signal_power_dbm: i8,
    /// p-b.
    pub p_b: u8,
    /// ul-CarrierFreq.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ul_earfcn: Option<u16>,
    /// ul-Bandwidth, RBs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ul_bw_rb: Option<u8>,
    /// numberOfRA-Preambles.
    pub ra_preambles: u8,
    /// modificationPeriodCoeff.
    pub mod_period_coeff: u8,
    /// defaultPagingCycle, frames.
    pub paging_cycle_frames: u16,
    /// t300, ms.
    pub t300_ms: u16,
    /// t310, ms.
    pub t310_ms: u16,
    /// n310.
    pub n310: u8,
    /// timeAlignmentTimerCommon, ms. `None` for infinity.
    #[serde(with = "time_alignment")]
    pub time_alignment: Option<u16>,
    /// Access class barring in force.
    pub ac_barring: bool,
}

/// SIB3 as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib3Out {
    /// q-Hyst, dB.
    pub q_hyst_db: u8,
    /// Priority.
    pub priority: u8,
    /// s-IntraSearch, dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s_intra_search_db: Option<u16>,
    /// s-NonIntraSearch, dB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s_non_intra_search_db: Option<u16>,
    /// threshServingLow, dB.
    pub thresh_serving_low_db: u16,
    /// t-Reselection, s.
    pub t_reselection_s: u8,
}

/// An intra-frequency neighbour.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Neigh {
    /// PCI.
    pub pci: u16,
    /// q-OffsetCell, dB.
    pub q_offset_db: i8,
}

/// An inter-frequency carrier.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InterOut {
    /// EARFCN.
    pub earfcn: u16,
    /// Bandwidth, RBs.
    pub bw_rb: u8,
    /// Priority.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// threshX-High, dB.
    pub thresh_high_db: u16,
    /// threshX-Low, dB.
    pub thresh_low_db: u16,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: i32,
    /// Neighbour PCIs.
    pub cells: Vec<u16>,
}

sdrkit_core::choice_enum! {
    /// The radio system of a carrier that a cell names for reselection.
    #[derive(Default, Serialize, Deserialize)]
    pub enum OtherSystem {
        /// UMTS FDD (SIB6).
        #[default]
        #[serde(rename = "umts_fdd")]
        UmtsFdd = ("umts_fdd", "UMTS FDD (SIB6)"),
        /// UMTS TDD (SIB6).
        #[serde(rename = "umts_tdd")]
        UmtsTdd = ("umts_tdd", "UMTS TDD (SIB6)"),
        /// GSM (SIB7).
        #[serde(rename = "gsm")]
        Gsm = ("gsm", "GSM (SIB7)"),
        /// CDMA2000 HRPD (SIB8).
        #[serde(rename = "cdma2000_hrpd")]
        Cdma2000Hrpd = ("cdma2000_hrpd", "CDMA2000 HRPD (SIB8)"),
        /// CDMA2000 1xRTT (SIB8).
        #[serde(rename = "cdma2000_1x")]
        Cdma2000OneX = ("cdma2000_1x", "CDMA2000 1xRTT (SIB8)"),
        /// 5G NR (SIB24).
        #[serde(rename = "nr")]
        Nr = ("nr", "5G NR (SIB24)"),
    }
}

/// A carrier of another radio system that a cell names in SIB6, SIB7, SIB8 or SIB24.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OtherCarrier {
    /// The system.
    pub system: OtherSystem,
    /// The channel number: UARFCN, GSM ARFCN, CDMA2000 ARFCN or NR-ARFCN.
    pub channel: u32,
    /// The band: "dcs1800" or "pcs1900" for GSM, "bc0" .. for CDMA2000, "n78 n77" for NR.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub band: Option<String>,
    /// cellReselectionPriority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,
    /// q-RxLevMin, dBm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub q_rx_lev_min_dbm: Option<i32>,
    /// The SSB spacing, kHz (NR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssb_scs_khz: Option<u16>,
    /// The cells named on the carrier: PN offsets (CDMA2000).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<u16>,
}

/// The carriers of other systems in one SIB: SIB6, SIB7, SIB8 or SIB24.
fn other_carriers(e: &SibEntry) -> Option<(&'static [OtherSystem], Vec<OtherCarrier>)> {
    let mut v = Vec::new();
    let systems: &'static [OtherSystem] = match e {
        SibEntry::Sib6(s) => {
            for (system, list) in [(OtherSystem::UmtsFdd, &s.fdd), (OtherSystem::UmtsTdd, &s.tdd)] {
                v.extend(list.iter().map(|c| OtherCarrier {
                    system,
                    channel: u32::from(c.uarfcn),
                    priority: c.priority,
                    q_rx_lev_min_dbm: Some(c.q_rx_lev_min_dbm()),
                    ..Default::default()
                }));
            }
            &[OtherSystem::UmtsFdd, OtherSystem::UmtsTdd]
        }
        SibEntry::Sib7(s) => {
            for g in &s.groups {
                // The band indicator has a meaning for ARFCNs 512 to 810 only.
                let band = |a: u16| (512..=810).contains(&a).then(|| if g.pcs1900 { "pcs1900" } else { "dcs1800" }.to_string());
                v.extend(g.arfcns().into_iter().map(|a| OtherCarrier {
                    system: OtherSystem::Gsm,
                    channel: u32::from(a),
                    band: band(a),
                    priority: g.priority,
                    q_rx_lev_min_dbm: Some(g.q_rx_lev_min_dbm()),
                    ..Default::default()
                }));
            }
            &[OtherSystem::Gsm]
        }
        SibEntry::Sib8(s) => {
            for (system, r) in [(OtherSystem::Cdma2000Hrpd, &s.hrpd), (OtherSystem::Cdma2000OneX, &s.one_x)] {
                for c in r.iter().flat_map(|r| &r.carriers) {
                    let priority = r.iter().flat_map(|r| &r.bands).find(|b| b.band_class == c.band_class).and_then(|b| b.priority);
                    v.push(OtherCarrier {
                        system,
                        channel: u32::from(c.arfcn),
                        band: Some(format!("bc{}", c.band_class)),
                        priority,
                        cells: c.pn.clone(),
                        ..Default::default()
                    });
                }
            }
            &[OtherSystem::Cdma2000Hrpd, OtherSystem::Cdma2000OneX]
        }
        SibEntry::Sib24(s) => {
            v.extend(s.carriers.iter().map(|c| OtherCarrier {
                system: OtherSystem::Nr,
                channel: c.nr_arfcn,
                band: (!c.bands.is_empty()).then(|| c.bands.iter().map(|b| format!("n{b}")).collect::<Vec<_>>().join(" ")),
                priority: c.priority,
                q_rx_lev_min_dbm: Some(c.q_rx_lev_min_dbm()),
                ssb_scs_khz: Some(c.ssb_scs_khz),
                ..Default::default()
            }));
            &[OtherSystem::Nr]
        }
        _ => return None,
    };
    Some((systems, v))
}

/// An LTE cell of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LteCell {
    /// PCI.
    pub pci: u16,
    /// The duplex.
    pub duplex: Duplex,
    /// The cyclic prefix.
    pub cyclic_prefix: Length,
    /// Nominal frequency, Hz.
    pub frequency_hz: f64,
    /// EARFCN, -1 when unknown.
    pub earfcn: i64,
    /// Band, 0 when none.
    pub band: u16,
    /// SIB1 confirmed the band.
    pub band_confirmed: bool,
    /// Other bands holding the frequency.
    pub bands_also: Vec<u16>,
    /// The other bands are SIB1's multi-band list.
    pub mfbi: bool,
    /// Carrier offset, Hz.
    pub carrier_offset_hz: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// MIB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mib: Option<MibOut>,
    /// SIB1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib1: Option<Sib1Out>,
    /// SIB2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib2: Option<Sib2Out>,
    /// SIB3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib3: Option<Sib3Out>,
    /// SIB4 neighbours.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib4_intra_neighbours: Option<Vec<Neigh>>,
    /// SIB5 carriers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib5_inter_frequencies: Option<Vec<InterOut>>,
    /// The carriers of other systems from SIB6, SIB7, SIB8 and SIB24.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub other_carriers: Vec<OtherCarrier>,
    /// SIBs seen.
    pub sibs_seen: Vec<u16>,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
    /// RSRQ, dB.
    pub rsrq_db: f64,
    /// SINR, dB.
    pub sinr_db: f64,
    /// PSS strength, dB.
    pub pss_db: f64,
    /// SI messages decoded.
    pub si_msgs: u64,
    /// Transport blocks that failed.
    pub si_crc_failures: u64,
    /// SIB1 schedules SIB10 or SIB11 (ETWS earthquake and tsunami warning).
    #[serde(default)]
    pub etws_scheduled: bool,
    /// SIB1 schedules SIB12 (CMAS public warning).
    #[serde(default)]
    pub cmas_scheduled: bool,
}

/// The list filters.
#[derive(Clone, Debug, PartialEq)]
pub struct LteFilter {
    /// Hidden bands.
    pub hide_bands: Vec<u16>,
    /// Only these bands.
    pub only_bands: Vec<u16>,
    /// Text search.
    pub search: String,
    /// Minimum RSRP (-120: off).
    pub min_rsrp: f64,
    /// Band confirmed by SIB1 only.
    pub band_known: bool,
    /// SIB1 read only.
    pub identified: bool,
    /// Leave out barred cells.
    pub not_barred: bool,
    /// One row per (primary PLMN, cell identity).
    pub one_per_cell: bool,
}

impl Default for LteFilter {
    fn default() -> Self {
        LteFilter {
            hide_bands: Vec::new(),
            only_bands: Vec::new(),
            search: String::new(),
            min_rsrp: MIN_RSRP_OFF,
            band_known: false,
            identified: false,
            not_barred: false,
            one_per_cell: false,
        }
    }
}

/// The value of `min_rsrp` that turns the RSRP filter off.
const MIN_RSRP_OFF: f64 = -120.0;

impl LteFilter {
    fn text(c: &LteCell) -> String {
        let mut s = String::new();
        if let Some(b) = &c.sib1 {
            for p in &b.plmns {
                s.push_str(p);
                s.push(' ');
            }
            if let Some(o) = &b.operator {
                s.push_str(o);
                s.push(' ');
            }
            s.push_str(&format!("enb {} tac {} id {} ", b.enb, b.tac, b.cell_id));
        }
        s.push_str(&format!("pci {}", c.pci));
        s.to_lowercase()
    }

    /// Whether the cell passes every filter except "one row per cell".
    pub fn keep(&self, c: &LteCell) -> bool {
        if self.hide_bands.contains(&c.band) {
            return false;
        }
        if !self.only_bands.is_empty() && (c.band == 0 || !self.only_bands.contains(&c.band)) {
            return false;
        }
        if !self.search.is_empty() && !Self::text(c).contains(&self.search.to_lowercase()) {
            return false;
        }
        if self.min_rsrp > MIN_RSRP_OFF && c.rsrp_dbfs < self.min_rsrp {
            return false;
        }
        if self.band_known && !c.band_confirmed {
            return false;
        }
        if self.identified && c.sib1.is_none() {
            return false;
        }
        !(self.not_barred && c.sib1.as_ref().is_some_and(|s| s.barred))
    }

    /// The cells listed, in the given order.
    pub fn apply<'a>(&self, cells: impl IntoIterator<Item = &'a LteCell>) -> Vec<&'a LteCell> {
        self.apply_tagged(cells.into_iter().map(|c| ((), c)))
            .into_iter()
            .map(|(_, c)| c)
            .collect()
    }

    /// As [`LteFilter::apply`], with a tag carried along with each cell.
    pub fn apply_tagged<'a, T>(
        &self,
        rows: impl IntoIterator<Item = (T, &'a LteCell)>,
    ) -> Vec<(T, &'a LteCell)> {
        let kept = rows.into_iter().filter(|(_, c)| self.keep(c));
        if !self.one_per_cell {
            return kept.collect();
        }
        let id = |c: &LteCell| {
            c.sib1
                .as_ref()
                .map(|s| (s.plmns.first().cloned().unwrap_or_default(), s.cell_id))
        };
        let mut out: Vec<(T, &LteCell)> = Vec::new();
        for (t, c) in kept {
            let Some(k) = id(c) else {
                out.push((t, c));
                continue;
            };
            match out.iter().position(|(_, o)| id(o).as_ref() == Some(&k)) {
                Some(i) if out[i].1.rsrp_dbfs < c.rsrp_dbfs => out[i] = (t, c),
                Some(_) => {}
                None => out.push((t, c)),
            }
        }
        out
    }
}

/// The most cells the table keeps.
pub const CELL_CAP: usize = 400;
/// The most carriers the carriers table lists.
pub const CARRIER_CAP: usize = 24;
/// The most rows of a table of cell details (several rows a cell).
pub(crate) const DETAIL_CAP: usize = 4 * CELL_CAP;

/// The key of a cell: its PCI and its channel on the 100 kHz raster (the offset from 0 Hz without the radio centre).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LteKey {
    /// PCI.
    pub pci: u16,
    /// The nominal frequency in steps of 100 kHz.
    pub raster: i64,
}

/// "301@18150" for PCI 301 at 1815.0 MHz.
impl std::fmt::Display for LteKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.pci, self.raster)
    }
}

/// What the LTE stack keeps and saves: the cells and the carriers of the newest capture.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LteTables {
    /// The cells.
    pub cells: ObjectTable<LteKey, LteCell>,
    /// The humps of the newest capture, strongest first.
    pub carriers: Vec<search::Hump>,
}

impl Default for LteTables {
    fn default() -> Self {
        LteTables {
            cells: ObjectTable::new(CELL_CAP),
            carriers: Vec::new(),
        }
    }
}

impl Shift for LteCell {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shift for LteTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for LteTables {
    const FORMAT: StateFormat = StateFormat::new("cell.lte_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
        self.carriers.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
        self.carriers = saved.carriers;
    }
}

sdrkit_core::choice_enum! {
    /// The list a neighbour comes from.
    pub enum NeighbourList {
        /// SIB4: a cell on the same frequency.
        Intra = ("intra", "Same frequency (SIB4)"),
        /// SIB5: a cell on another carrier.
        Inter = ("inter", "Other carrier (SIB5)"),
        /// SIB8: a CDMA2000 cell.
        Cdma2000 = ("cdma2000", "CDMA2000 (SIB8)"),
    }
}

/// The country of the primary MCC of SIB1.
fn country_of(s: &Sib1Out) -> Option<&'static str> {
    mcc::country_short(s.mcc)
}

impl Shaped for LteCell {
    type Row = CellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let s = self.sib1.as_ref();
        out.put(&CellRow {
            plmn: s.and_then(|s| s.plmns.first()).map(String::as_str),
            operator: s.and_then(|s| s.operator.as_deref()),
            country: s.and_then(country_of),
            plmn_count: s.map(|s| s.plmns.len() as i64),
            band: (self.band > 0).then_some(i64::from(self.band)),
            band_confirmed: self.band_confirmed,
            earfcn: (self.earfcn >= 0).then_some(self.earfcn),
            frequency: Hertz(self.frequency_hz),
            pci: i64::from(self.pci),
            duplex: self.duplex,
            enb: s.map(|s| i64::from(s.enb)),
            sector: s.map(|s| i64::from(s.sector)),
            cell_id: s.map(|s| i64::from(s.cell_id)),
            tac: s.map(|s| i64::from(s.tac)),
            width: self.mib.as_ref().map(|m| Hertz(m.bandwidth_mhz * 1e6)),
            ports: self.mib.as_ref().map(|m| m.ports as i64),
            rsrp_dbfs: self.rsrp_dbfs,
            rsrq_db: self.rsrq_db,
            sinr_db: self.sinr_db,
            carrier_offset: Hertz(self.carrier_offset_hz),
            barred: s.map(|s| s.barred),
            etws: self.etws_scheduled,
            cmas: self.cmas_scheduled,
            last_heard: columns::last_heard(seen).map(Time),
            generation: crate::scan::Generation::Fourth,
            name: &self.survey("").label(),
        });
    }
}

impl LteCell {
    /// The band choice the cell holds.
    pub fn choice(&self) -> BandChoice {
        BandChoice {
            band: self.band,
            confirmed: self.band_confirmed,
            also: self.bands_also.clone(),
            mfbi: self.mfbi,
        }
    }

    /// The details row of the cell whose row key is `key`.
    fn details<'a>(&self, key: &'a str) -> CellDetailRow<'a> {
        let ms = |v: u32| Seconds(f64::from(v) * 1e-3);
        let m = self.mib.as_ref();
        let s1 = self.sib1.as_ref();
        let s2 = self.sib2.as_ref();
        let s3 = self.sib3.as_ref();
        CellDetailRow {
            cell: key,
            cyclic_prefix: self.cyclic_prefix,
            phich_duration: m.map(|m| m.phich_duration),
            phich_ng: m.map(|m| i64::from(m.phich_ng_index)),
            sfn: m.map(|m| i64::from(m.sfn_first_whole_frame)),
            clock_ppm: self.clock_ppm,
            si_messages: self.si_msgs as i64,
            si_failed: self.si_crc_failures as i64,
            q_rx_lev_min_dbm: s1.map(|s| f64::from(s.q_rx_lev_min_dbm)),
            p_max_dbm: s1.and_then(|s| s.p_max_dbm).map(f64::from),
            si_window: s1.map(|s| ms(u32::from(s.si_window_ms))),
            value_tag: s1.map(|s| i64::from(s.value_tag)),
            ref_signal_power_dbm: s2.map(|s| f64::from(s.ref_signal_power_dbm)),
            p_b: s2.map(|s| i64::from(s.p_b)),
            ul_earfcn: s2.and_then(|s| s.ul_earfcn).map(i64::from),
            ul_rbs: s2.and_then(|s| s.ul_bw_rb).map(i64::from),
            ra_preambles: s2.map(|s| i64::from(s.ra_preambles)),
            mod_period_coeff: s2.map(|s| i64::from(s.mod_period_coeff)),
            paging_cycle: s2.map(|s| i64::from(s.paging_cycle_frames)),
            t300: s2.map(|s| ms(u32::from(s.t300_ms))),
            t310: s2.map(|s| ms(u32::from(s.t310_ms))),
            n310: s2.map(|s| i64::from(s.n310)),
            time_alignment: s2.and_then(|s| s.time_alignment).map(|v| ms(u32::from(v))),
            ac_barring: s2.map(|s| s.ac_barring),
            q_hyst_db: s3.map(|s| f64::from(s.q_hyst_db)),
            priority: s3.map(|s| i64::from(s.priority)),
            s_intra_search_db: s3.and_then(|s| s.s_intra_search_db).map(f64::from),
            s_non_intra_search_db: s3.and_then(|s| s.s_non_intra_search_db).map(f64::from),
            thresh_serving_low_db: s3.map(|s| f64::from(s.thresh_serving_low_db)),
            t_reselection: s3.map(|s| Seconds(f64::from(s.t_reselection_s))),
        }
    }

    /// The cell as a row of the survey: the first network of SIB1.
    pub fn survey(&self, key: &str) -> crate::scan::SurveyCell {
        let s = self.sib1.as_ref();
        crate::scan::SurveyCell {
            key: key.to_string(),
            generation: crate::scan::Generation::Fourth,
            id_word: "PCI",
            plmn: s.and_then(|s| s.plmns.first().cloned()),
            operator_name: s.and_then(|s| s.operator.clone()),
            band: (self.band > 0).then(|| format!("B{}", self.band)),
            channel: (self.earfcn >= 0).then_some(self.earfcn),
            frequency_hz: self.frequency_hz,
            pci: i64::from(self.pci),
            cell_identity: s.map(|s| i64::from(s.cell_id)),
            area_code: s.map(|s| i64::from(s.tac)),
            level_dbfs: Some(self.rsrp_dbfs),
        }
    }

    /// Writes the rows of the cell in the tables of its PLMNs, bands, SIBs, neighbours and other carriers.
    fn write_children(&self, key: &str, t: &mut ChildTables, p: &mut Published) {
        if let Some(s) = &self.sib1 {
            for (i, plmn) in s.plmns.iter().enumerate() {
                let row = CellPlmnRow {
                    cell: key,
                    plmn,
                    operator: Plmn::parse(plmn).and_then(mcc::operator),
                };
                t.plmns.put(p, format!("{key}/{i}"), &row);
            }
        }
        for &b in &self.bands_also {
            let row = CellBandRow {
                cell: key,
                band: i64::from(b),
                mfbi: self.mfbi,
            };
            t.bands.put(p, format!("{key}/{b}"), &row);
        }
        let mut sibs: Vec<(u16, Option<u16>)> = self.sibs_seen.iter().map(|&n| (n, None)).collect();
        if let Some(s) = &self.sib1 {
            for e in &s.scheduling {
                for &n in &e.sibs {
                    match sibs.iter_mut().find(|x| x.0 == n) {
                        Some(x) => x.1 = Some(e.period_frames),
                        None => sibs.push((n, Some(e.period_frames))),
                    }
                }
            }
        }
        for (n, period) in sibs {
            let row = CellSibRow {
                cell: key,
                sib: i64::from(n),
                period: period.map(i64::from),
                seen: self.sibs_seen.contains(&n),
            };
            t.sibs.put(p, format!("{key}/{n}"), &row);
        }
        let own = (self.earfcn >= 0).then_some(self.earfcn);
        for n in self.sib4_intra_neighbours.iter().flatten() {
            let row = CellNeighbourRow {
                cell: key,
                list: NeighbourList::Intra,
                earfcn: own,
                pci: i64::from(n.pci),
                q_offset_db: Some(f64::from(n.q_offset_db)),
                cdma2000_arfcn: None,
            };
            t.neighbours.put(p, format!("{key}/intra/{}", n.pci), &row);
        }
        for f in self.sib5_inter_frequencies.iter().flatten() {
            let row = CellInterCarrierRow {
                cell: key,
                earfcn: i64::from(f.earfcn),
                rbs: i64::from(f.bw_rb),
                priority: f.priority.map(i64::from),
                thresh_high_db: f64::from(f.thresh_high_db),
                thresh_low_db: f64::from(f.thresh_low_db),
                q_rx_lev_min_dbm: f64::from(f.q_rx_lev_min_dbm),
            };
            t.carriers.put(p, format!("{key}/{}", f.earfcn), &row);
            for &pci in &f.cells {
                let row = CellNeighbourRow {
                    cell: key,
                    list: NeighbourList::Inter,
                    earfcn: Some(i64::from(f.earfcn)),
                    pci: i64::from(pci),
                    q_offset_db: None,
                    cdma2000_arfcn: None,
                };
                t.neighbours.put(p, format!("{key}/{}/{pci}", f.earfcn), &row);
            }
        }
        for o in &self.other_carriers {
            let frequency = match o.system {
                OtherSystem::Nr => sdrkit_data_plans::cellular::nr_arfcn_hz(o.channel),
                OtherSystem::Gsm if o.band.as_deref() != Some("pcs1900") => {
                    sdrkit_data_plans::cellular::channel_hz(sdrkit_data_plans::cellular::CellSystem::Gsm, o.channel).map(|x| x.0)
                }
                _ => None,
            };
            let row = CellOtherCarrierRow {
                cell: key,
                system: o.system,
                channel: i64::from(o.channel),
                frequency: frequency.map(Hertz),
                band: o.band.as_deref(),
                ssb_scs: o.ssb_scs_khz.map(|k| Hertz(f64::from(k) * 1e3)),
                priority: o.priority.map(i64::from),
                q_rx_lev_min_dbm: o.q_rx_lev_min_dbm.map(f64::from),
                cells: o.cells.len() as i64,
            };
            let id = format!("{key}/{}/{}", <OtherSystem as sdrkit_core::Choice>::CHOICES[usize::from(sdrkit_core::Choice::index(o.system))].key, o.channel);
            t.others.put(p, id.clone(), &row);
            for &pn in &o.cells {
                let row = CellNeighbourRow {
                    cell: key,
                    list: NeighbourList::Cdma2000,
                    earfcn: None,
                    pci: i64::from(pn),
                    q_offset_db: None,
                    cdma2000_arfcn: Some(i64::from(o.channel)),
                };
                t.neighbours.put(p, format!("{id}/{pn}"), &row);
            }
        }
    }
}

/// The tables written from the listed cells.
struct ChildTables {
    plmns: RowSync<CellPlmnRow<'static>>,
    bands: RowSync<CellBandRow<'static>>,
    sibs: RowSync<CellSibRow<'static>>,
    neighbours: RowSync<CellNeighbourRow<'static>>,
    carriers: RowSync<CellInterCarrierRow<'static>>,
    others: RowSync<CellOtherCarrierRow<'static>>,
}

sdrkit_core::settings! {
    /// The settings of the LTE search component.
    pub enum LteSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// Decode SIB1 to SIB8 and SIB24.
        SystemInformation = SYSTEM_INFORMATION,
        /// Cells searched per carrier.
        MaxCells = MAX_CELLS,
        /// The PSS threshold.
        MinPssDb = MIN_PSS_DB,
        /// The carrier offset searched.
        MaxCfoKhz = MAX_CFO_KHZ,
        /// Bands left out of the list.
        HideBands = HIDE_BANDS,
        /// Bands the list keeps.
        OnlyBands = ONLY_BANDS,
        /// Text the listed cells contain.
        Search = SEARCH,
        /// The least RSRP listed.
        MinRsrp = MIN_RSRP,
        /// List cells whose band SIB1 confirmed only.
        BandKnown = BAND_KNOWN,
        /// List cells with SIB1 only.
        Identified = IDENTIFIED_ONLY,
        /// Leave out barred cells.
        NotBarred = NOT_BARRED,
        /// One row per cell identity.
        OnePerCell = ONE_PER_CELL,
        /// The bands taken first where bands overlap.
        BandOrder = BAND_ORDER,
        /// When the stack searches its bands.
        BandSearch = BAND_SEARCH,
        /// The bands a band search covers.
        SearchBands = SEARCH_BANDS,
        /// The least time on each radio centre of a band search.
        SearchDwell = SEARCH_DWELL,
    }
}

/// The default capture: two whole frames and the SIB1 repetition within 20 ms.
const CAPTURE_DEFAULT_S: f64 = 0.2;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// Decode SIB1 to SIB8 and SIB24.
pub const SYSTEM_INFORMATION: Setting<field::Bool> =
    Setting::new("system_information", "Read system information", "", true)
        .help("Decode SIB1 to SIB8 and SIB24 (needs the whole carrier in the sample band).");
/// Cells searched per carrier.
pub const MAX_CELLS: Setting<field::I64> =
    Setting::new("max_cells", "Cells per carrier", "", 12).range(1.0, 24.0, 1.0);
/// The PSS threshold.
pub const MIN_PSS_DB: Setting<field::F64> =
    Setting::new("min_pss_db", "PSS threshold", "dB", 9.0).range(6.0, 20.0, 0.5);
/// The carrier offset searched.
pub const MAX_CFO_KHZ: Setting<field::F64> =
    Setting::new("max_cfo_khz", "Carrier offset searched", "kHz", 25.0).range(5.0, 60.0, 1.0);
/// The section of the list filters.
const LIST_SECTION: &str = "List";
/// Bands left out of the list: numbers apart by spaces or commas.
pub const HIDE_BANDS: Setting<field::Text> = Setting::new("hide_bands", "Hide bands", "", "")
    .section(LIST_SECTION)
    .help("Band numbers to leave out (\"14 28, 64\").");
/// Bands the list keeps: numbers apart by spaces or commas. Empty keeps every band.
pub const ONLY_BANDS: Setting<field::Text> =
    Setting::new("only_bands", "Only bands", "", "").section(LIST_SECTION);
/// Text the listed cells contain.
pub const SEARCH: Setting<field::Text> =
    Setting::new("search", "Search", "", "").section(LIST_SECTION);
/// The least RSRP listed. The lowest value lists every cell.
pub const MIN_RSRP: Setting<field::F64> =
    Setting::new("min_rsrp", "Minimum RSRP", "dBFS", MIN_RSRP_OFF)
        .range(MIN_RSRP_OFF, -20.0, 1.0)
        .section(LIST_SECTION);
/// List cells whose band SIB1 confirmed only.
pub const BAND_KNOWN: Setting<field::Bool> =
    Setting::new("band_known", "Band confirmed only", "", false).section(LIST_SECTION);
/// List cells with SIB1 only.
pub const IDENTIFIED_ONLY: Setting<field::Bool> =
    Setting::new("identified", "Identified only", "", false).section(LIST_SECTION);
/// Leave out barred cells.
pub const NOT_BARRED: Setting<field::Bool> =
    Setting::new("not_barred", "Leave out barred cells", "", false).section(LIST_SECTION);
/// One row per cell identity: the strongest.
pub const ONE_PER_CELL: Setting<field::Bool> =
    Setting::new("one_per_cell", "One row per cell", "", false).section(LIST_SECTION);

/// The LTE search: what a capture holds and how the cells are kept.
pub struct LteSearch {
    searcher: LteSearcher,
    capture_s: f64,
    hide: BandList,
    only: BandList,
    order: BandList,
    /// The list filters.
    pub filter: LteFilter,
    /// The band search settings.
    scan: SearchConfig,
    /// Why the newest capture showed no cell.
    quiet: Option<String>,
}

impl Default for LteSearch {
    fn default() -> Self {
        LteSearch {
            searcher: LteSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            hide: BandList::default(),
            only: BandList::default(),
            order: BandList::default(),
            filter: LteFilter::default(),
            scan: SearchConfig::new(),
            quiet: None,
        }
    }
}

/// Why a capture of LTE `out` at `rate` showed no cell, for people; None when it showed one.
fn quiet_reason(out: &LteSearchOut, rate: f64, threshold_db: f64, max_cfo_hz: f64, centre_hz: f64) -> Option<String> {
    if !out.cells.is_empty() {
        return None;
    }
    if rate < MIN_RATE_HZ {
        return Some(format!("the capture is too narrow for LTE: it needs {:.2} Msps", MIN_RATE_HZ / 1e6));
    }
    let best = if out.best_pss_db.is_finite() {
        format!("{:.1} dB", out.best_pss_db)
    } else {
        "none".to_owned()
    };
    if out.best_pss_db >= threshold_db {
        return Some(format!(
            "LTE sync seen (PSS {best}) but no cell identity read: the signal is too weak, or the frequency error is too large"
        ));
    }
    match out.humps.iter().max_by(|a, b| a.level_db.total_cmp(&b.level_db)) {
        Some(h) => Some(format!(
            "a carrier at {:.1} MHz ({:.1} MHz wide, {:.0} dB over the floor) shows no LTE sync (strongest {best}, {threshold_db:.0} dB \
             needed): not LTE, or a frequency error beyond the {:.0} kHz searched",
            (centre_hz + h.offset_hz) / 1e6,
            h.width_hz / 1e6,
            h.level_db,
            max_cfo_hz / 1e3
        )),
        None => Some(format!(
            "no LTE carrier on this frequency: {} carrier positions tried, the strongest sync {best} ({threshold_db:.0} dB \
             needed); a band search finds the carriers",
            out.candidates
        )),
    }
}

impl LteSearch {
    fn merge_cell(
        &mut self,
        t: &mut LteTables,
        cap: &Capture,
        f: &search::LteFound,
        log: &mut Log<Self>,
    ) {
        let time = cap.end_time;
        let known = cap.centre_known();
        let actual = cap.centre_hz + f.cand.offset_hz + f.cfo_hz;
        let (earfcn, nominal) = if known {
            let e = nearest_earfcn(actual / 1e6, None, &self.order.bands);
            let nom = if e >= 0 {
                earfcn_to_mhz(e as u32) * 1e6
            } else {
                (actual / 100e3).round() * 100e3
            };
            (e, nom)
        } else {
            (-1, f.cand.offset_hz)
        };
        if known && earfcn < 0 && f.mib.is_none() {
            return;
        }
        let key = LteKey {
            pci: f.pci,
            raster: (nominal / 100e3).round() as i64,
        };
        let Ok(up) = t.cells.upsert(&key, log.stamp(time), || {
            let choice = if known {
                band_choice(nominal / 1e6, 0, &self.order.bands)
            } else {
                BandChoice::default()
            };
            LteCell {
                pci: f.pci,
                frequency_hz: nominal,
                earfcn,
                band: choice.band,
                band_confirmed: choice.confirmed,
                bands_also: choice.also,
                mfbi: choice.mfbi,
                ..Default::default()
            }
        }) else {
            return;
        };
        let c = up.row;
        c.duplex = if f.tdd { Duplex::Tdd } else { Duplex::Fdd };
        c.cyclic_prefix = if f.ext {
            Length::Extended
        } else {
            Length::Normal
        };
        if let Some((m, ports, sfn0)) = &f.mib {
            let n_rb = phy::N_RB[m.bw_index as usize];
            c.mib = Some(MibOut {
                n_rb,
                bandwidth_mhz: n_rb as f64 * 0.2,
                ports: *ports,
                phich_duration: if m.phich_extended {
                    Length::Extended
                } else {
                    Length::Normal
                },
                phich_ng_index: m.ng_index,
                sfn_first_whole_frame: *sfn0,
            });
        }
        c.rsrp_dbfs = f.rsrp_dbfs;
        c.rsrq_db = f.rsrq_db;
        c.sinr_db = f.sinr_db;
        c.pss_db = f.pss_db;
        c.carrier_offset_hz = if known { actual - nominal } else { f.cfo_hz };
        c.clock_ppm = f.clock_ppm;
        c.si_crc_failures += f.crc_failures as u64;
        *log.failed += f.crc_failures as u64;
        if up.new {
            let place = if known {
                format!("{:.1} MHz (EARFCN {})", nominal / 1e6, earfcn)
            } else {
                format!("{:+.1} kHz", nominal / 1e3)
            };
            let text = match &c.mib {
                Some(m) => format!(
                    "PCI {} at {place}, {} MHz, {} ports",
                    c.pci, m.bandwidth_mhz, m.ports
                ),
                None => format!("PCI {} at {place}, synchronisation signals only", c.pci),
            };
            log.event(time, key.to_string(), text);
        }
        for (_, _, _, tb) in &f.tbs {
            let Some(msg) = Bcch::parse(tb) else { continue };
            *log.decoded += 1;
            c.si_msgs += 1;
            apply_bcch(c, &key, msg, time, nominal, &self.order.bands, log);
        }
    }

    /// The values of the band list setting `s`, or a refusal with the position of the bad character.
    fn band_list(
        id: SettingId,
        v: &Value,
        s: &Setting<field::Text>,
    ) -> Result<BandList, SettingError> {
        BandList::read(id, s.read(id, v)?)
    }
}

impl SearchStack for LteSearch {
    type Found = LteSearchOut;
    type Tables = LteTables;
    type Setting = LteSetting;
    const NAME: &'static str = "cell.lte_search";
    const PREFIX: &'static str = "cell: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.lte_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = CELL_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &CELL_LTE_CELL;

    fn tables(&self) -> LteTables {
        LteTables::default()
    }
    fn apply(&mut self, s: LteSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let r = &mut self.searcher;
        let f = &mut self.filter;
        match s {
            LteSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            LteSetting::SystemInformation => {
                r.system_information = SYSTEM_INFORMATION.read(id, v)?
            }
            LteSetting::MaxCells => r.max_cells = MAX_CELLS.read(id, v)? as usize,
            LteSetting::MinPssDb => r.min_pss_db = MIN_PSS_DB.read(id, v)?,
            LteSetting::MaxCfoKhz => r.max_cfo_hz = MAX_CFO_KHZ.read(id, v)? * 1e3,
            LteSetting::HideBands => {
                self.hide = Self::band_list(id, v, &HIDE_BANDS)?;
                f.hide_bands = self.hide.bands.clone();
            }
            LteSetting::OnlyBands => {
                self.only = Self::band_list(id, v, &ONLY_BANDS)?;
                f.only_bands = self.only.bands.clone();
            }
            LteSetting::Search => f.search = SEARCH.read(id, v)?.to_owned(),
            LteSetting::MinRsrp => f.min_rsrp = MIN_RSRP.read(id, v)?,
            LteSetting::BandKnown => f.band_known = BAND_KNOWN.read(id, v)?,
            LteSetting::Identified => f.identified = IDENTIFIED_ONLY.read(id, v)?,
            LteSetting::NotBarred => f.not_barred = NOT_BARRED.read(id, v)?,
            LteSetting::OnePerCell => f.one_per_cell = ONE_PER_CELL.read(id, v)?,
            LteSetting::BandOrder => self.order = Self::band_list(id, v, &BAND_ORDER)?,
            LteSetting::BandSearch => self.scan.mode = BAND_SEARCH.read(id, v)?,
            LteSetting::SearchBands => self.scan.bands = Self::band_list(id, v, &SEARCH_BANDS)?,
            LteSetting::SearchDwell => self.scan.dwell_s = SEARCH_DWELL.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let s = &self.searcher;
        let f = &self.filter;
        values_of([
            (LteSetting::CaptureS, Value::F64(self.capture_s)),
            (
                LteSetting::SystemInformation,
                Value::Bool(s.system_information),
            ),
            (LteSetting::MaxCells, Value::I64(s.max_cells as i64)),
            (LteSetting::MinPssDb, Value::F64(s.min_pss_db)),
            (LteSetting::MaxCfoKhz, Value::F64(s.max_cfo_hz / 1e3)),
            (LteSetting::HideBands, Value::Text(self.hide.text.clone())),
            (LteSetting::OnlyBands, Value::Text(self.only.text.clone())),
            (LteSetting::Search, Value::Text(f.search.clone())),
            (LteSetting::MinRsrp, Value::F64(f.min_rsrp)),
            (LteSetting::BandKnown, Value::Bool(f.band_known)),
            (LteSetting::Identified, Value::Bool(f.identified)),
            (LteSetting::NotBarred, Value::Bool(f.not_barred)),
            (LteSetting::OnePerCell, Value::Bool(f.one_per_cell)),
            (LteSetting::BandOrder, Value::Text(self.order.text.clone())),
            (LteSetting::BandSearch, self.scan.mode_value()),
            (LteSetting::SearchBands, Value::Text(self.scan.bands.text.clone())),
            (LteSetting::SearchDwell, Value::F64(self.scan.dwell_s)),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> LteSearchOut {
        self.searcher.search(cap)
    }
    fn merge(
        &mut self,
        t: &mut LteTables,
        cap: &Capture,
        found: LteSearchOut,
        log: &mut Log<Self>,
    ) {
        let base = if cap.centre_known() {
            cap.centre_hz
        } else {
            0.0
        };
        t.carriers = found
            .humps
            .iter()
            .take(CARRIER_CAP)
            .map(|h| search::Hump {
                offset_hz: base + h.offset_hz,
                ..h.clone()
            })
            .collect();
        self.quiet = quiet_reason(&found, cap.rate, self.searcher.min_pss_db, self.searcher.max_cfo_hz, base);
        for f in &found.cells {
            self.merge_cell(t, cap, f, log);
        }
    }
    fn found_cells(found: &LteSearchOut) -> usize {
        found.cells.len()
    }
    fn quiet_reason(&self) -> Option<String> {
        self.quiet.clone()
    }
    fn band_search(&self) -> Option<&SearchConfig> {
        Some(&self.scan)
    }
    fn search_steps(&self, band: u16, rate: f64) -> Vec<f64> {
        crate::scan::lte_steps(band, rate)
    }
    fn bands_at(&self, hz: f64) -> Vec<u16> {
        crate::bands::downlink_bands(sdrkit_data_plans::cellular::CellSystem::Lte, hz, &self.order.bands)
    }
    fn carriers(t: &LteTables) -> Vec<FoundCarrier> {
        t.cells
            .iter()
            .filter(|r| r.record.frequency_hz > 0.0)
            .map(|r| FoundCarrier {
                row: r.key.to_string(),
                frequency_hz: r.record.frequency_hz,
                level_dbfs: r.record.rsrp_dbfs,
                name: format!("PCI {} at {:.1} MHz", r.record.pci, r.record.frequency_hz / 1e6),
            })
            .collect()
    }
    fn remove(tables: &mut Self::Tables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut tables.cells, row)
    }
    fn forget_capture(tables: &mut LteTables) {
        tables.carriers.clear();
    }
    fn publish(&self, t: &LteTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        let ident = t.cells.iter().filter(|r| r.record.sib1.is_some()).count();
        let mut ops: Vec<&String> = t
            .cells
            .iter()
            .filter_map(|r| r.record.sib1.as_ref()?.plmns.first())
            .collect();
        ops.sort();
        ops.dedup();
        p.put(&CellFields {
            cell_count: n as i64,
            identified: ident as i64,
            operators: ops.len() as i64,
            banner: BANNER_TEXT,
        });
        // The listed cells, after the filters.
        let listed = self
            .filter
            .apply_tagged(t.cells.iter().map(|r| (*r.key, r.record)));
        let keys: HashSet<LteKey> = listed.iter().map(|(k, _)| *k).collect();
        shaped::rows_where(&t.cells, &CELL_CELLS, now, p, |r| keys.contains(r.key));
        write_listed(&listed, p);
        shaped::indexed_rows(p, &CELL_CARRIERS, &t.carriers, |h, out| {
            out.put(&CellCarrierRow {
                frequency: Hertz(h.offset_hz),
                width: Hertz(h.width_hz),
                level_db: h.level_db,
            });
        });
        // The strongest cell by RSRP among those seen within the recent window.
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.rsrp_dbfs.total_cmp(&b.record.rsrp_dbfs))
            .map(|r| r.record);
        Headline {
            summary: format!(
                "{n} cell{}, {ident} identified, {} operator{}",
                plural(n),
                ops.len(),
                plural(ops.len())
            ),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|c| c.sinr_db as f32),
            offset_hz: best.map(|c| c.carrier_offset_hz),
        }
    }
}

/// Writes the tables that follow the listed cells: details, networks, bands, SIBs, neighbours, other carriers, sites and
/// operators.
fn write_listed(listed: &[(LteKey, &LteCell)], p: &mut Published) {
    let mut details = RowSync::new(&CELL_DETAILS);
    let mut children = ChildTables {
        plmns: RowSync::new(&CELL_PLMNS),
        bands: RowSync::new(&CELL_BANDS),
        sibs: RowSync::new(&CELL_SIBS),
        neighbours: RowSync::new(&CELL_NEIGHBOURS),
        carriers: RowSync::new(&CELL_INTER_CARRIERS),
        others: RowSync::new(&CELL_OTHER_CARRIERS),
    };
    let mut sites: Vec<(String, &Sib1Out, i64)> = Vec::new();
    let mut operators: Vec<(&str, &Sib1Out, i64)> = Vec::new();
    let mut survey = Vec::new();
    for (key, c) in listed {
        let key = key.to_string();
        survey.push(c.survey(&key));
        let row = c.details(&key);
        details.put(p, key.clone(), &row);
        c.write_children(&key, &mut children, p);
        let Some(s) = &c.sib1 else { continue };
        let plmn = s.plmns.first().map_or("", String::as_str);
        let site = format!("{plmn}/{}", s.enb);
        match sites.iter_mut().find(|x| x.0 == site) {
            Some(x) => x.2 += 1,
            None => sites.push((site, s, 1)),
        }
        match operators.iter_mut().find(|x| x.0 == plmn) {
            Some(x) => x.2 += 1,
            None => operators.push((plmn, s, 1)),
        }
    }
    details.finish(p);
    children.plmns.finish(p);
    children.bands.finish(p);
    children.sibs.finish(p);
    children.neighbours.finish(p);
    children.carriers.finish(p);
    children.others.finish(p);
    crate::scan::write_survey(&crate::typed::CELL_SURVEY, survey, p);
    let mut rows = RowSync::new(&CELL_SITES);
    for (key, s, n) in sites {
        let row = CellSiteRow {
            plmn: s.plmns.first().map(String::as_str),
            operator: s.operator.as_deref(),
            enb: i64::from(s.enb),
            tac: i64::from(s.tac),
            cell_count: n,
        };
        rows.put(p, key, &row);
    }
    rows.finish(p);
    let mut rows = RowSync::new(&CELL_OPERATORS);
    for (plmn, s, n) in operators {
        let row = CellOperatorRow {
            plmn,
            operator: s.operator.as_deref(),
            country: country_of(s),
            cell_count: n,
        };
        rows.put(p, plmn.to_owned(), &row);
    }
    rows.finish(p);
}

/// Applies one BCCH message to `c`, the cell with key `key` (TS 36.331 6.2.2).
fn apply_bcch(
    c: &mut LteCell,
    key: &LteKey,
    msg: Bcch,
    t: f64,
    nominal: f64,
    order: &[u16],
    log: &mut Log<LteSearch>,
) {
    let see = |c: &mut LteCell, s: u16| {
        if let Err(at) = c.sibs_seen.binary_search(&s) {
            c.sibs_seen.insert(at, s);
        }
    };
    match msg {
        Bcch::Sib1(s) => {
            see(c, 1);
            let mut plmns = Vec::new();
            let mut prev = None;
            let mut first = (0u16, 0u16);
            for (i, (p, _)) in s.plmns.iter().enumerate() {
                let (mcc, mnc, d) = p.numbers(prev);
                prev = p.mcc.or(prev);
                if i == 0 {
                    first = (mcc, mnc);
                }
                plmns.push(
                    Plmn {
                        mcc,
                        mnc,
                        mnc_digits: d,
                    }
                    .to_string(),
                );
            }
            let op = plmns
                .first()
                .and_then(|p| Plmn::parse(p).and_then(mcc::operator))
                .map(String::from);
            let enb = s.cell_id >> 8;
            let sector = s.cell_id & 255;
            let changed = c
                .sib1
                .as_ref()
                .is_none_or(|o| o.cell_id != s.cell_id || o.tac != s.tac);
            let mut choice = band_choice(nominal / 1e6, s.band, order);
            apply_mfbi(&mut choice, &s.multi_band);
            c.band = choice.band;
            c.band_confirmed = choice.confirmed;
            c.bands_also = choice.also;
            c.mfbi = choice.mfbi;
            if c.band > 0 {
                if let Some((n, _)) = lte_band(c.band).and_then(|b| b.nearest_channel(nominal)) {
                    c.earfcn = n.into();
                }
            }
            let sibs = || s.scheduling.iter().flat_map(|(_, v)| v.iter().copied());
            c.etws_scheduled = sibs().any(|n| n == 10 || n == 11);
            c.cmas_scheduled = sibs().any(|n| n == 12);
            if changed {
                let text = format!(
                    "PCI {}: {} {}, TAC {}, eNB {} sector {}, band {}",
                    c.pci,
                    plmns.first().cloned().unwrap_or_default(),
                    op.clone().unwrap_or_default(),
                    s.tac,
                    enb,
                    sector,
                    c.band
                );
                log.event(t, key.to_string(), text.replace("  ", " "));
            }
            c.sib1 = Some(Sib1Out {
                plmns,
                operator: op,
                tac: s.tac,
                cell_id: s.cell_id,
                enb,
                sector,
                band: c.band,
                barred: s.barred,
                q_rx_lev_min_dbm: s.q_rx_lev_min_dbm,
                p_max_dbm: s.p_max_dbm,
                si_window_ms: s.si_window_ms,
                value_tag: s.value_tag,
                scheduling: s
                    .scheduling
                    .iter()
                    .map(|(p, v)| SchedOut {
                        period_frames: *p,
                        sibs: v.clone(),
                    })
                    .collect(),
                mcc: first.0,
                mnc: first.1,
            });
        }
        Bcch::Si(entries) => {
            for e in entries {
                match e {
                    SibEntry::Sib2(s) => {
                        see(c, 2);
                        let barred = s.ac_barring.as_ref().is_some_and(|(_, a, b)| {
                            a.is_some_and(|x| x.factor != 15) || b.is_some_and(|x| x.factor != 15)
                        });
                        c.sib2 = Some(Sib2Out {
                            ref_signal_power_dbm: s.ref_signal_power,
                            p_b: s.p_b,
                            ul_earfcn: s.ul_earfcn,
                            ul_bw_rb: s.ul_bw_rb,
                            ra_preambles: s.ra_preambles,
                            mod_period_coeff: s.mod_period_coeff,
                            paging_cycle_frames: s.paging_cycle,
                            t300_ms: s.timers.0,
                            t310_ms: s.timers.2,
                            n310: s.timers.3,
                            time_alignment: s.time_alignment,
                            ac_barring: barred,
                        });
                    }
                    SibEntry::Sib3(s) => {
                        see(c, 3);
                        c.sib3 = Some(Sib3Out {
                            q_hyst_db: s.q_hyst_db,
                            priority: s.priority,
                            s_intra_search_db: s.s_intra.map(|v| 2 * v as u16),
                            s_non_intra_search_db: s.s_non_intra.map(|v| 2 * v as u16),
                            thresh_serving_low_db: 2 * s.thresh_serving_low as u16,
                            t_reselection_s: s.t_reselection,
                        });
                    }
                    SibEntry::Sib4(s) => {
                        see(c, 4);
                        c.sib4_intra_neighbours = Some(
                            s.neighbours
                                .iter()
                                .map(|&(pci, q)| Neigh {
                                    pci,
                                    q_offset_db: q,
                                })
                                .collect(),
                        );
                    }
                    SibEntry::Sib5(s) => {
                        see(c, 5);
                        c.sib5_inter_frequencies = Some(
                            s.carriers
                                .iter()
                                .map(|f| InterOut {
                                    earfcn: f.earfcn,
                                    bw_rb: phy::N_RB[f.allowed_bw as usize % 6] as u8,
                                    priority: f.priority,
                                    thresh_high_db: 2 * f.thresh_high as u16,
                                    thresh_low_db: 2 * f.thresh_low as u16,
                                    q_rx_lev_min_dbm: f.q_rx_lev_min_dbm,
                                    cells: f.neighbours.iter().map(|n| n.0).collect(),
                                })
                                .collect(),
                        );
                    }
                    SibEntry::Sib6(_) | SibEntry::Sib7(_) | SibEntry::Sib8(_) | SibEntry::Sib24(_) => {
                        let n = match e {
                            SibEntry::Sib6(_) => 6,
                            SibEntry::Sib7(_) => 7,
                            SibEntry::Sib8(_) => 8,
                            _ => 24,
                        };
                        see(c, n);
                        if let Some((systems, v)) = other_carriers(&e) {
                            c.other_carriers.retain(|o| !systems.contains(&o.system));
                            c.other_carriers.extend(v);
                        }
                    }
                    SibEntry::Other(0) => {}
                    SibEntry::Other(n) => see(c, n),
                }
            }
        }
    }
}

/// The settings of the `cell` stack: the search settings and the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The LTE receiver: one search component, the stack settings routed to it.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(
        LteSearch::default(),
        MIN_RATE_HZ,
        "LTE cells needs a sample rate of at least 1.92 Msps",
        cfg,
    )
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the generator of the test signal: the clean signal only.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(gen::from_config(cfg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::parse_band_list;

    fn cell(pci: u16, band: u16, rsrp: f64, sib: Option<(&str, &str, u32, bool)>) -> LteCell {
        LteCell {
            pci,
            band,
            band_confirmed: sib.is_some(),
            rsrp_dbfs: rsrp,
            sib1: sib.map(|(p1, p2, id, barred)| Sib1Out {
                plmns: vec![p1.into(), p2.into()],
                operator: Plmn::parse(p1).and_then(mcc::operator).map(String::from),
                cell_id: id,
                enb: id >> 8,
                barred,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn filters() {
        let cells = vec![
            cell(1, 20, -60.0, Some(("246-01", "246-02", 1000, false))),
            cell(2, 20, -50.0, Some(("246-01", "246-03", 1000, false))),
            cell(3, 3, -90.0, Some(("310-410", "001-01", 2000, true))),
            cell(4, 7, -100.0, None),
        ];
        let f = LteFilter::default();
        assert_eq!(f.apply(&cells).len(), 4);
        let pcis = |f: &LteFilter| f.apply(&cells).iter().map(|c| c.pci).collect::<Vec<_>>();
        assert_eq!(
            pcis(&LteFilter {
                hide_bands: parse_band_list("20, 7").unwrap(),
                ..Default::default()
            }),
            vec![3]
        );
        assert_eq!(
            pcis(&LteFilter {
                only_bands: vec![3],
                ..Default::default()
            }),
            vec![3]
        );
        assert_eq!(
            pcis(&LteFilter {
                search: "246-03".into(),
                ..Default::default()
            }),
            vec![2]
        );
        assert_eq!(
            pcis(&LteFilter {
                search: "at&t".into(),
                ..Default::default()
            }),
            vec![3]
        );
        assert_eq!(
            pcis(&LteFilter {
                search: "pci 4".into(),
                ..Default::default()
            }),
            vec![4]
        );
        assert_eq!(
            pcis(&LteFilter {
                min_rsrp: -70.0,
                ..Default::default()
            }),
            vec![1, 2]
        );
        assert_eq!(
            pcis(&LteFilter {
                band_known: true,
                ..Default::default()
            }),
            vec![1, 2, 3]
        );
        assert_eq!(
            pcis(&LteFilter {
                identified: true,
                ..Default::default()
            }),
            vec![1, 2, 3]
        );
        assert_eq!(
            pcis(&LteFilter {
                not_barred: true,
                ..Default::default()
            }),
            vec![1, 2, 4]
        );
        assert_eq!(
            pcis(&LteFilter {
                one_per_cell: true,
                ..Default::default()
            }),
            vec![2, 3, 4]
        );
        assert_eq!(
            pcis(&LteFilter {
                one_per_cell: true,
                not_barred: true,
                min_rsrp: -95.0,
                ..Default::default()
            }),
            vec![2]
        );
        // NR filters.
        let mut c = nr_cell(78, -60.0, true, false);
        let nf = crate::nr::NrFilter {
            min_rsrp: -120.0,
            ..Default::default()
        };
        assert!(nf.keep(&c));
        assert!(!crate::nr::NrFilter {
            hide_bands: vec![78],
            min_rsrp: -120.0,
            ..Default::default()
        }
        .keep(&c));
        assert!(!crate::nr::NrFilter {
            min_rsrp: -50.0,
            ..Default::default()
        }
        .keep(&c));
        c.mib = None;
        assert!(!crate::nr::NrFilter {
            mib_only: true,
            min_rsrp: -120.0,
            ..Default::default()
        }
        .keep(&c));
        let b = nr_cell(78, -60.0, true, true);
        assert!(!crate::nr::NrFilter {
            not_barred: true,
            min_rsrp: -120.0,
            ..Default::default()
        }
        .keep(&b));
    }

    fn nr_cell(band: u16, rsrp: f64, mib: bool, barred: bool) -> crate::nr::NrCell {
        crate::nr::NrCell {
            band,
            rsrp_dbfs: rsrp,
            mib: mib.then(|| crate::nr::MibOut {
                barred,
                ..Default::default()
            }),
            ..Default::default()
        }
    }
}
