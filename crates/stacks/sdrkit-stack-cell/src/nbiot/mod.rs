//! The NB-IoT cells stack (`nbiot`): NPSS and NSSS, MIB-NB, and SIB1-NB on SI-RNTI.
//!
//! Standard: 3GPP TS 36.211 clause 10, TS 36.331.

pub mod gen;
pub mod msg;
pub mod phy;
pub mod search;

use crate::bands::{earfcn_band, mhz_to_earfcn};
use crate::decoder::{
    build_search, capture_setting, plural, BandList, Capture, Headline, Log, SearchStack, BAND_ORDER,
};
use crate::typed::{NbiotCellRow, NbiotFields, NBIOT_CELL, NBIOT_CELLS};
use sdrkit_core::interface::{EventKind, Hertz, Time};
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
use search::{NbSearchOut, NbSearcher};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "nbiot";
/// The lowest input rate, Hz: one 180 kHz carrier with room for the raster and tuning offsets.
pub const MIN_RATE_HZ: f64 = 1e6;

sdrkit_core::choice_enum! {
    /// How an NB-IoT carrier is deployed relative to LTE (TS 36.331 MasterInformationBlock-NB operationModeInfo).
    #[derive(Default, Serialize, Deserialize)]
    pub enum OperationMode {
        /// In-band, with the PCI of the LTE cell.
        #[serde(rename = "in-band (same PCI)")]
        InbandSamePci = ("inband_same_pci", "In-band (same PCI)"),
        /// In-band, with another PCI.
        #[serde(rename = "in-band (other PCI)")]
        InbandDifferentPci = ("inband_different_pci", "In-band (other PCI)"),
        /// In the guard band of an LTE carrier.
        #[serde(rename = "guard band")]
        Guardband = ("guardband", "Guard band"),
        /// Standalone.
        #[default]
        #[serde(rename = "standalone")]
        Standalone = ("standalone", "Standalone"),
    }
}

impl MibOut {
    /// The label of the operation mode, for log lines.
    fn mode_label(&self) -> &'static str {
        use sdrkit_core::Choice;
        &OperationMode::CHOICES[usize::from(self.operation_mode.index())].label
    }
}

impl From<&msg::OperationMode> for OperationMode {
    fn from(m: &msg::OperationMode) -> Self {
        match m {
            msg::OperationMode::InbandSamePci(_) => OperationMode::InbandSamePci,
            msg::OperationMode::InbandDifferentPci { .. } => OperationMode::InbandDifferentPci,
            msg::OperationMode::Guardband(_) => OperationMode::Guardband,
            msg::OperationMode::Standalone => OperationMode::Standalone,
        }
    }
}

/// The MIB-NB as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MibOut {
    /// The operation mode.
    pub operation_mode: OperationMode,
    /// NRS ports.
    pub nrs_ports: usize,
    /// SFN of the capture's frame 0.
    pub sfn: u32,
    /// schedulingInfoSIB1.
    pub scheduling_info_sib1: u8,
    /// systemInfoValueTag.
    pub value_tag: u8,
    /// Hyper SFN LSBs.
    pub hsfn_lsb: u8,
    /// ab-Enabled.
    pub access_barring: bool,
}

/// The SIB1-NB as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1Out {
    /// PLMNs.
    pub plmns: Vec<String>,
    /// Operator of the first PLMN.
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
    /// q-QualMin, dB.
    pub q_qual_min_db: i32,
    /// p-Max, dBm.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p_max_dbm: Option<i32>,
    /// Hyper SFN MSBs.
    pub hsfn_msb: u8,
}

/// An NB-IoT cell of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NbCell {
    /// PCI.
    pub pci: u16,
    /// Nominal frequency, Hz.
    pub frequency_hz: f64,
    /// EARFCN, -1 when none.
    pub earfcn: i64,
    /// Band, 0 when none.
    pub band: u16,
    /// Raster offset, kHz.
    pub raster_offset_khz: f64,
    /// Tuning error, Hz.
    pub tuning_error_hz: f64,
    /// Total carrier offset (actual - nominal), Hz.
    pub total_carrier_offset_hz: f64,
    /// The status carrier offset (the tuning error).
    pub carrier_offset_hz: f64,
    /// MIB-NB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mib: Option<MibOut>,
    /// SIB1-NB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sib1: Option<Sib1Out>,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// NPSS score.
    pub npss_score: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// SIB1-NB messages read.
    pub sib1_msgs: u64,
}

/// The most cells the table keeps.
pub const CELL_CAP: usize = 400;

/// The key of a cell: its PCI and its channel on the 100 kHz raster (the offset from 0 Hz without the radio centre).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NbKey {
    /// PCI.
    pub pci: u16,
    /// The nominal frequency in steps of 100 kHz.
    pub raster: i64,
}

/// "100@9450" for PCI 100 at 945.0 MHz.
impl std::fmt::Display for NbKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}@{}", self.pci, self.raster)
    }
}

/// What the NB-IoT stack keeps and saves: the cells.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NbTables {
    /// The cells.
    pub cells: ObjectTable<NbKey, NbCell>,
}

impl Default for NbTables {
    fn default() -> Self {
        NbTables {
            cells: ObjectTable::new(CELL_CAP),
        }
    }
}

impl Shift for NbCell {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shift for NbTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for NbTables {
    const FORMAT: StateFormat = StateFormat::new("cell.nbiot_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
    }
}

impl sdrkit_graph::Item for NbCell {
    const KIND: &'static str = "cell.nbiot_cell";
}

impl Shaped for NbCell {
    type Row = NbiotCellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let s = self.sib1.as_ref();
        out.put(&NbiotCellRow {
            plmn: s.and_then(|s| s.plmns.first()).map(String::as_str),
            operator: s.and_then(|s| s.operator.as_deref()),
            band: (self.band > 0).then_some(i64::from(self.band)),
            earfcn: (self.earfcn >= 0).then_some(self.earfcn),
            frequency: Hertz(self.frequency_hz),
            pci: i64::from(self.pci),
            operation_mode: self.mib.as_ref().map(|m| m.operation_mode),
            enb: s.map(|s| i64::from(s.enb)),
            sector: s.map(|s| i64::from(s.sector)),
            tac: s.map(|s| i64::from(s.tac)),
            barred: s.map(|s| s.barred),
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            last_heard: columns::last_heard(seen).map(Time),
            generation: crate::scan::Generation::Fourth,
            name: &crate::scan::cell_name(
                s.and_then(|s| s.operator.as_deref().or(s.plmns.first().map(String::as_str))),
                crate::scan::Generation::Fourth,
                Some("NB-IoT"),
                &format!("PCI {}", self.pci),
            ),
        });
    }
}

/// The NB-IoT search: what a capture holds and how the cells are kept.
pub struct NbSearch {
    searcher: NbSearcher,
    capture_s: f64,
    /// The radio's known frequency error, ppm.
    correction_ppm: f64,
    order: BandList,
}

impl Default for NbSearch {
    fn default() -> Self {
        NbSearch {
            searcher: NbSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            correction_ppm: 0.0,
            order: BandList::default(),
        }
    }
}

impl NbSearch {
    fn merge_cell(
        &mut self,
        t: &mut NbTables,
        cap: &Capture,
        f: &search::NbFound,
        log: &mut Log<Self>,
    ) {
        let time = cap.end_time;
        let known = cap.centre_known();
        let actual = cap.centre_hz + f.candidate_hz + f.cfo_hz;
        let nominal = if known {
            (actual / 100e3).round() * 100e3
        } else {
            f.candidate_hz
        };
        let earfcn = if known {
            mhz_to_earfcn(nominal / 1e6, None, &self.order.bands)
        } else {
            -1
        };
        let total = actual - nominal;
        let stated = f.mib.as_ref().and_then(|m| m.0.mode.raster_offset_khz());
        // The raster offset: as the MIB-NB states it, else the nearest of the five offsets of 36.101 5.7.3 once the known
        // frequency error is taken out, else the whole offset.
        let raster_khz = match stated {
            Some(k) => k,
            None if self.correction_ppm != 0.0 => {
                let known_err = self.correction_ppm * 1e-6 * actual;
                let r = (total - known_err) / 1e3;
                [-7.5f64, -2.5, 0.0, 2.5, 7.5]
                    .into_iter()
                    .min_by(|a, b| (a - r).abs().total_cmp(&(b - r).abs()))
                    .unwrap_or(0.0)
            }
            None => total / 1e3,
        };
        let key = NbKey {
            pci: f.pci,
            raster: (nominal / 100e3).round() as i64,
        };
        let Ok(up) = t.cells.upsert(&key, log.stamp(time), || NbCell {
            pci: f.pci,
            frequency_hz: nominal,
            earfcn,
            band: if earfcn >= 0 {
                earfcn_band(earfcn as u32)
            } else {
                0
            },
            ..Default::default()
        }) else {
            return;
        };
        let new = up.new;
        let c = up.row;
        c.raster_offset_khz = raster_khz;
        c.total_carrier_offset_hz = total;
        c.tuning_error_hz = total - raster_khz * 1e3;
        c.carrier_offset_hz = c.tuning_error_hz;
        c.level_dbfs = f.level_dbfs;
        c.snr_db = f.snr_db;
        c.npss_score = f.npss_score;
        c.clock_ppm = f.clock_ppm;
        if let Some((m, ports, sfn0)) = &f.mib {
            c.mib = Some(MibOut {
                operation_mode: OperationMode::from(&m.mode),
                nrs_ports: *ports,
                sfn: *sfn0,
                scheduling_info_sib1: m.scheduling_info_sib1,
                value_tag: m.value_tag,
                hsfn_lsb: m.hsfn_lsb,
                access_barring: m.access_barring,
            });
            *log.decoded += 1;
        }
        *log.failed += f.sib1_failed as u64;
        let mode = c.mib.as_ref().map(|m| m.mode_label());
        let place = if known {
            format!("{:.1} MHz (EARFCN {})", nominal / 1e6, c.earfcn)
        } else {
            format!("{:+.1} kHz", nominal / 1e3)
        };
        if new {
            let text = match &mode {
                Some(m) => format!("PCI {} at {place}, {m}", c.pci),
                None => format!("PCI {} at {place}, synchronisation signals only", c.pci),
            };
            log.event(time, key.to_string(), text);
        }
        let Some(s) = &f.sib1 else {
            return;
        };
        *log.decoded += 1;
        c.sib1_msgs += 1;
        let mut plmns = Vec::new();
        let mut prev = None;
        for (p, _) in &s.plmns {
            let (mcc, mnc, d) = p.numbers(prev);
            prev = p.mcc.or(prev);
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
        let first_sib = c.sib1.is_none();
        if first_sib {
            let text = format!(
                "PCI {}: {} {}, TAC {}, eNB {} sector {}, band {}, {}",
                c.pci,
                plmns.first().cloned().unwrap_or_default(),
                op.clone().unwrap_or_default(),
                s.tac,
                s.cell_id >> 8,
                s.cell_id & 255,
                s.band,
                mode.unwrap_or_default()
            );
            log.event(time, key.to_string(), text.replace("  ", " "));
        }
        c.sib1 = Some(Sib1Out {
            plmns,
            operator: op,
            tac: s.tac,
            cell_id: s.cell_id,
            enb: s.cell_id >> 8,
            sector: s.cell_id & 255,
            band: s.band,
            barred: s.barred,
            q_rx_lev_min_dbm: s.q_rx_lev_min_dbm,
            q_qual_min_db: s.q_qual_min_db,
            p_max_dbm: s.p_max_dbm,
            hsfn_msb: s.hsfn_msb,
        });
        c.band = s.band;
    }
}

impl SearchStack for NbSearch {
    type Found = NbSearchOut;
    type Tables = NbTables;
    type Setting = NbSetting;
    const NAME: &'static str = "cell.nbiot_search";
    const PREFIX: &'static str = "nbiot: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.nbiot_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = NBIOT_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &NBIOT_CELL;

    fn tables(&self) -> NbTables {
        NbTables::default()
    }
    fn apply(&mut self, s: NbSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match s {
            NbSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            NbSetting::MinNpss => self.searcher.min_npss = MIN_NPSS.read(id, v)?,
            NbSetting::ReadSib1 => self.searcher.read_sib1 = READ_SIB1.read(id, v)?,
            NbSetting::FreqCorrectionPpm => {
                self.correction_ppm = FREQ_CORRECTION_PPM.read(id, v)?
            }
            NbSetting::BandOrder => self.order = BandList::read(id, BAND_ORDER.read(id, v)?)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (NbSetting::CaptureS, Value::F64(self.capture_s)),
            (NbSetting::MinNpss, Value::F64(self.searcher.min_npss)),
            (NbSetting::ReadSib1, Value::Bool(self.searcher.read_sib1)),
            (
                NbSetting::FreqCorrectionPpm,
                Value::F64(self.correction_ppm),
            ),
            (NbSetting::BandOrder, Value::Text(self.order.text.clone())),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> NbSearchOut {
        self.searcher.search(cap)
    }
    fn merge(&mut self, t: &mut NbTables, cap: &Capture, found: NbSearchOut, log: &mut Log<Self>) {
        for f in &found.cells {
            self.merge_cell(t, cap, f, log);
        }
    }
    fn remove(tables: &mut Self::Tables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut tables.cells, row)
    }
    fn publish(&self, t: &NbTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        let ident = t.cells.iter().filter(|r| r.record.sib1.is_some()).count();
        p.put(&NbiotFields {
            cell_count: n as i64,
            identified: ident as i64,
        });
        shaped::rows(&t.cells, &NBIOT_CELLS, now, p);
        // The strongest cell by level among those seen within the recent window.
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.level_dbfs.total_cmp(&b.record.level_dbfs))
            .map(|r| r.record);
        Headline {
            summary: format!("{n} NB-IoT cell{}, {ident} identified", plural(n)),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|c| c.snr_db as f32),
            offset_hz: best.map(|c| c.carrier_offset_hz),
        }
    }
}

sdrkit_core::settings! {
    /// The settings of the NB-IoT search component.
    pub enum NbSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// The NPSS threshold.
        MinNpss = MIN_NPSS,
        /// Read SIB1-NB.
        ReadSib1 = READ_SIB1,
        /// The radio's known frequency error.
        FreqCorrectionPpm = FREQ_CORRECTION_PPM,
        /// The bands taken first where bands overlap.
        BandOrder = BAND_ORDER,
    }
}

/// The default capture: eight NPBCH repetitions and a SIB1-NB period need about a second of signal.
const CAPTURE_DEFAULT_S: f64 = 1.0;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// The NPSS threshold.
pub const MIN_NPSS: Setting<field::F64> =
    Setting::new("min_npss", "NPSS threshold", "", 3.6).range(2.0, 10.0, 0.1);
/// Read SIB1-NB.
pub const READ_SIB1: Setting<field::Bool> = Setting::new("read_sib1", "Read SIB1-NB", "", true);
/// The radio's known frequency error. It splits a carrier offset into raster offset and tuning error.
pub const FREQ_CORRECTION_PPM: Setting<field::F64> =
    Setting::new("freq_correction_ppm", "Radio frequency correction", "ppm", 0.0)
        .range(-100.0, 100.0, 0.0)
        .help("The radio's known frequency error. It splits a carrier offset into raster offset and tuning error.");

/// The settings of the `nbiot` stack: the search settings and the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The NB-IoT receiver: one search component, the stack settings routed to it.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(
        NbSearch::default(),
        MIN_RATE_HZ,
        "4G NB-IoT cells needs a sample rate of at least 1 Msps",
        cfg,
    )
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the generator of the test signal: the clean signal only.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::from_config(cfg)))
}
