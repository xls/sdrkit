//! The 5G NR cells stack (`nr`): the SS/PBCH blocks, the MIB and the beams of every cell on the synchronisation raster.
//!
//! Standard: 3GPP TS 38.211, TS 38.212, TS 38.213 and TS 38.331.

pub mod gen;
pub mod phy;
pub mod search;

use crate::bands::{coreset0, hz_to_gscn, nr_band_of};
use crate::decoder::{
    build_search, capture_setting, plural, BandList, Capture, Headline, Log, SearchStack, BAND_ORDER,
};
use crate::typed::{NrCellRow, NrFields, NR_CELL, NR_CELLS};
use sdrkit_core::interface::{EventKind, Hertz, Time};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, GenConfig, Generator, Receiver, Schema, Setting, SettingError,
    SettingId, StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format as StateFormat, ObjectTable, Seen, Shift};
use search::{NrSearchOut, NrSearcher};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "nr";
/// The radio centre that test scenes 0 and 2 assume, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 3541.44e6;
/// The lowest input rate, Hz: a 15 kHz SS/PBCH block is 3.6 MHz wide.
pub const MIN_RATE_HZ: f64 = 3.84e6;

/// The MIB as published.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MibOut {
    /// SFN.
    pub sfn: u16,
    /// Half frame bit.
    pub hrf: u8,
    /// subCarrierSpacingCommon, kHz (15 or 30 in FR1).
    pub scs_common_khz: u32,
    /// k_SSB.
    pub k_ssb: u8,
    /// DMRS type A position (2 or 3).
    pub dmrs_type_a_position: u8,
    /// controlResourceSetZero.
    pub coreset0: u8,
    /// searchSpaceZero.
    pub search_space0: u8,
    /// Barred.
    pub barred: bool,
    /// Intra-frequency reselection allowed.
    pub intra_freq_reselection: bool,
}

/// A beam.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BeamOut {
    /// SSB index.
    pub ssb: u8,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
}

/// An NR cell of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NrCell {
    /// PCI.
    pub pci: u16,
    /// N_ID1.
    pub nid1: u16,
    /// N_ID2.
    pub nid2: u16,
    /// SSB spacing, kHz.
    pub ssb_scs_khz: u32,
    /// GSCN, -1 without the centre.
    pub gscn: i64,
    /// Band (n number), 0 when none.
    pub band: u16,
    /// SS_REF (nominal), Hz.
    pub frequency_hz: f64,
    /// Carrier offset relative to SS_REF, Hz.
    pub carrier_offset_hz: f64,
    /// MIB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mib: Option<MibOut>,
    /// k_SSB <= 23.
    pub sib1_scheduled: bool,
    /// CORESET#0 RBs.
    pub coreset0_rbs: u32,
    /// CORESET#0 symbols.
    pub coreset0_symbols: u32,
    /// CORESET#0 RB offset.
    pub coreset0_offset_rb: u32,
    /// Lmax.
    pub lmax: u8,
    /// The strongest SSB.
    pub strongest_ssb: u8,
    /// Beams by index.
    pub beams: Vec<BeamOut>,
    /// RSRP, dBFS.
    pub rsrp_dbfs: f64,
    /// RSRQ, dB.
    pub rsrq_db: f64,
    /// SINR, dB.
    pub sinr_db: f64,
    /// PSS strength, dB.
    pub pss_db: f64,
    /// MIB decodes.
    pub mibs: u64,
}

/// The NR list filters.
#[derive(Clone, Debug, PartialEq)]
pub struct NrFilter {
    /// Bands hidden.
    pub hide_bands: Vec<u16>,
    /// Only these bands.
    pub only_bands: Vec<u16>,
    /// Minimum RSRP (-120: off).
    pub min_rsrp: f64,
    /// Only cells with a MIB.
    pub mib_only: bool,
    /// Leave out barred cells.
    pub not_barred: bool,
}

impl Default for NrFilter {
    fn default() -> Self {
        NrFilter {
            hide_bands: Vec::new(),
            only_bands: Vec::new(),
            min_rsrp: MIN_RSRP_OFF,
            mib_only: false,
            not_barred: false,
        }
    }
}

/// The value of `min_rsrp` that turns the RSRP filter off.
const MIN_RSRP_OFF: f64 = -120.0;

impl NrFilter {
    /// Whether a cell is listed.
    pub fn keep(&self, c: &NrCell) -> bool {
        if self.hide_bands.contains(&c.band) {
            return false;
        }
        if !self.only_bands.is_empty() && (c.band == 0 || !self.only_bands.contains(&c.band)) {
            return false;
        }
        if self.min_rsrp > MIN_RSRP_OFF && c.rsrp_dbfs < self.min_rsrp {
            return false;
        }
        if self.mib_only && c.mib.is_none() {
            return false;
        }
        !(self.not_barred && c.mib.as_ref().is_some_and(|m| m.barred))
    }
}

/// The most cells the table keeps.
pub const CELL_CAP: usize = 300;

/// The key of a cell: its PCI, its SSB spacing and its SS_REF in steps of 30 kHz.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NrKey {
    /// PCI.
    pub pci: u16,
    /// SSB spacing, kHz.
    pub scs_khz: u32,
    /// SS_REF in steps of 30 kHz.
    pub raster: i64,
}

/// "222/15@26528" for PCI 222 with a 15 kHz SSB at 795.84 MHz.
impl std::fmt::Display for NrKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}@{}", self.pci, self.scs_khz, self.raster)
    }
}

/// What the NR stack keeps and saves: the cells and what the newest capture searched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NrTables {
    /// The cells.
    pub cells: ObjectTable<NrKey, NrCell>,
    /// The lowest and highest GSCN the newest capture searched.
    pub gscn_range: Option<(u32, u32)>,
    /// The candidates the newest capture searched.
    pub candidates: Option<usize>,
}

impl Default for NrTables {
    fn default() -> Self {
        NrTables {
            cells: ObjectTable::new(CELL_CAP),
            gscn_range: None,
            candidates: None,
        }
    }
}

impl Shift for NrCell {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shift for NrTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for NrTables {
    const FORMAT: StateFormat = StateFormat::new("cell.nr_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
        self.gscn_range = None;
        self.candidates = None;
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
        self.gscn_range = saved.gscn_range;
        self.candidates = saved.candidates;
    }
}

impl sdrkit_graph::Item for NrCell {
    const KIND: &'static str = "cell.nr_cell";
}

/// The notice the stack publishes.
pub const BANNER_TEXT: &str =
    "Receive only: the public information every NR cell broadcasts to all phones (cell identity, \
beams, MIB). Nothing addressed to a phone is decoded.";

impl Shaped for NrCell {
    type Row = NrCellRow;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&NrCellRow {
            band: (self.band > 0).then_some(i64::from(self.band)),
            gscn: (self.gscn >= 0).then_some(self.gscn),
            frequency: Hertz(self.frequency_hz),
            pci: i64::from(self.pci),
            ssb_scs: Hertz(f64::from(self.ssb_scs_khz) * 1e3),
            beams: self.beams.len() as i64,
            sfn: self.mib.as_ref().map(|m| i64::from(m.sfn)),
            sib1: self.sib1_scheduled,
            rsrp_dbfs: self.rsrp_dbfs,
            rsrq_db: self.rsrq_db,
            sinr_db: self.sinr_db,
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// The NR search: what a capture holds and how the cells are kept.
pub struct NrSearch {
    searcher: NrSearcher,
    capture_s: f64,
    hide: BandList,
    only: BandList,
    order: BandList,
    /// The list filters.
    pub filter: NrFilter,
}

impl Default for NrSearch {
    fn default() -> Self {
        NrSearch {
            searcher: NrSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            hide: BandList::default(),
            only: BandList::default(),
            order: BandList::default(),
            filter: NrFilter::default(),
        }
    }
}

impl NrSearch {
    fn merge_cell(
        &mut self,
        t: &mut NrTables,
        cap: &Capture,
        f: &search::NrFound,
        log: &mut Log<Self>,
    ) {
        let time = cap.end_time;
        *log.decoded += f.pbch_ok as u64;
        *log.failed += (f.pbch_tried - f.pbch_ok) as u64;
        if f.mib.is_none() && f.significance < 6.0 {
            return;
        }
        let known = cap.centre_known();
        let nominal = if known {
            cap.centre_hz + f.cand.offset_hz
        } else {
            f.cand.offset_hz
        };
        let scs = f.cand.scs;
        let key = NrKey {
            pci: f.pci,
            scs_khz: scs,
            raster: (nominal / 30e3).round() as i64,
        };
        let Ok(up) = t.cells.upsert(&key, log.stamp(time), || {
            let gscn = if known {
                f.cand.gscn.or_else(|| hz_to_gscn(nominal))
            } else {
                None
            };
            NrCell {
                pci: f.pci,
                nid1: f.pci / 3,
                nid2: f.pci % 3,
                ssb_scs_khz: scs,
                gscn: gscn.map_or(-1, i64::from),
                band: gscn
                    .and_then(|g| nr_band_of(g, Some(scs), &self.order.bands))
                    .map_or(0, |b| b.number()),
                frequency_hz: nominal,
                ..Default::default()
            }
        }) else {
            return;
        };
        let new = up.new;
        let c = up.row;
        let had_mib = c.mib.is_some();
        if let Some((m, _, _)) = &f.mib {
            let scs_common = if m.scs_common_30 { 30 } else { 15 };
            c.mib = Some(MibOut {
                sfn: m.sfn,
                hrf: m.hrf,
                scs_common_khz: scs_common,
                k_ssb: m.k_ssb,
                dmrs_type_a_position: if m.dmrs_pos3 { 3 } else { 2 },
                coreset0: m.coreset0,
                search_space0: m.search_space0,
                barred: m.barred,
                intra_freq_reselection: m.intra_freq_allowed,
            });
            c.lmax = f.lmax.unwrap_or(8);
            c.sib1_scheduled = m.k_ssb <= 23;
            let cs = if c.sib1_scheduled {
                coreset0(scs, scs_common, m.coreset0 as usize)
            } else {
                None
            };
            (c.coreset0_rbs, c.coreset0_symbols, c.coreset0_offset_rb) = cs.unwrap_or((0, 0, 0));
            c.mibs += 1;
        }
        // Beams merged by index; only indices below Lmax.
        for b in &f.beams {
            match c.beams.iter_mut().find(|x| x.ssb == b.ssb) {
                Some(x) => x.rsrp_dbfs = b.rsrp_dbfs,
                None => c.beams.push(BeamOut {
                    ssb: b.ssb,
                    rsrp_dbfs: b.rsrp_dbfs,
                }),
            }
        }
        if c.mib.is_none() {
            c.beams.clear();
        }
        let lmax = if c.lmax == 0 { 8 } else { c.lmax };
        c.beams.retain(|b| b.ssb < lmax);
        c.beams.sort_by_key(|b| b.ssb);
        if let Some(b) = c
            .beams
            .iter()
            .max_by(|a, b| a.rsrp_dbfs.total_cmp(&b.rsrp_dbfs))
        {
            c.strongest_ssb = b.ssb;
        }
        c.rsrp_dbfs = f.rsrp_dbfs;
        c.rsrq_db = f.rsrq_db;
        c.sinr_db = f.sinr_db;
        c.pss_db = f.pss_db;
        c.carrier_offset_hz = f.cfo_hz;
        let place = if known {
            format!("{:.2} MHz (GSCN {}, n{})", nominal / 1e6, c.gscn, c.band)
        } else {
            format!("{:+.1} kHz", nominal / 1e3)
        };
        match &c.mib {
            None if new => log.event(
                time,
                key.to_string(),
                format!(
                    "PCI {} at {place}, {} kHz SSB, synchronisation signals only",
                    c.pci, scs
                ),
            ),
            Some(m) if new || !had_mib => {
                let text = format!(
                    "PCI {} at {place}, {} kHz SSB, SSB {}, SFN {}, {}{}",
                    c.pci,
                    scs,
                    c.strongest_ssb,
                    m.sfn,
                    if c.sib1_scheduled {
                        "SIB1 scheduled"
                    } else {
                        "no SIB1 (k_SSB > 23)"
                    },
                    if m.barred { ", barred" } else { "" }
                );
                log.event(time, key.to_string(), text);
            }
            _ => {}
        }
    }
}

impl SearchStack for NrSearch {
    type Found = NrSearchOut;
    type Tables = NrTables;
    type Setting = NrSetting;
    const NAME: &'static str = "cell.nr_search";
    const PREFIX: &'static str = "nr: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.nr_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = NR_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &NR_CELL;

    fn tables(&self) -> NrTables {
        NrTables::default()
    }
    fn apply(&mut self, s: NrSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        let f = &mut self.filter;
        match s {
            NrSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            NrSetting::MaxCells => self.searcher.max_cells = MAX_CELLS.read(id, v)? as usize,
            NrSetting::MinPssDb => self.searcher.min_pss_db = MIN_PSS_DB.read(id, v)?,
            NrSetting::HideBands => {
                self.hide = BandList::read(id, HIDE_BANDS.read(id, v)?)?;
                f.hide_bands = self.hide.bands.clone();
            }
            NrSetting::OnlyBands => {
                self.only = BandList::read(id, ONLY_BANDS.read(id, v)?)?;
                f.only_bands = self.only.bands.clone();
            }
            NrSetting::MinRsrp => f.min_rsrp = MIN_RSRP.read(id, v)?,
            NrSetting::MibOnly => f.mib_only = MIB_ONLY.read(id, v)?,
            NrSetting::NotBarred => f.not_barred = NOT_BARRED.read(id, v)?,
            NrSetting::BandOrder => {
                self.order = BandList::read(id, BAND_ORDER.read(id, v)?)?;
                self.searcher.band_order = self.order.bands.clone();
            }
        }
        Ok(())
    }
    fn values(&self) -> Values {
        let f = &self.filter;
        values_of([
            (NrSetting::CaptureS, Value::F64(self.capture_s)),
            (
                NrSetting::MaxCells,
                Value::I64(self.searcher.max_cells as i64),
            ),
            (NrSetting::MinPssDb, Value::F64(self.searcher.min_pss_db)),
            (NrSetting::HideBands, Value::Text(self.hide.text.clone())),
            (NrSetting::OnlyBands, Value::Text(self.only.text.clone())),
            (NrSetting::MinRsrp, Value::F64(f.min_rsrp)),
            (NrSetting::MibOnly, Value::Bool(f.mib_only)),
            (NrSetting::NotBarred, Value::Bool(f.not_barred)),
            (NrSetting::BandOrder, Value::Text(self.order.text.clone())),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> NrSearchOut {
        self.searcher.search(cap)
    }
    fn merge(&mut self, t: &mut NrTables, cap: &Capture, found: NrSearchOut, log: &mut Log<Self>) {
        t.gscn_range = found.gscn_range;
        t.candidates = Some(found.candidates);
        for f in &found.cells {
            self.merge_cell(t, cap, f, log);
        }
    }
    fn remove(tables: &mut Self::Tables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut tables.cells, row)
    }
    fn forget_capture(tables: &mut NrTables) {
        tables.gscn_range = None;
        tables.candidates = None;
    }
    fn publish(&self, t: &NrTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        let with_mib = t.cells.iter().filter(|r| r.record.mib.is_some()).count();
        let beams: usize = t.cells.iter().map(|r| r.record.beams.len()).sum();
        p.put(&NrFields {
            cell_count: n as i64,
            with_mib: with_mib as i64,
            gscn_lo: t.gscn_range.map(|r| i64::from(r.0)),
            gscn_hi: t.gscn_range.map(|r| i64::from(r.1)),
            candidates: t.candidates.map(|c| c as i64),
            banner: BANNER_TEXT,
        });
        shaped::rows_where(&t.cells, &NR_CELLS, now, p, |r| self.filter.keep(r.record));
        // The strongest cell by RSRP among those seen within the recent window.
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.rsrp_dbfs.total_cmp(&b.record.rsrp_dbfs))
            .map(|r| r.record);
        Headline {
            summary: format!(
                "{n} NR cell{}, {with_mib} with MIB, {beams} beam{}",
                plural(n),
                plural(beams)
            ),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|c| c.sinr_db as f32),
            offset_hz: best.map(|c| c.carrier_offset_hz),
        }
    }
}

sdrkit_core::settings! {
    /// The settings of the NR search component.
    pub enum NrSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// Cells searched per candidate.
        MaxCells = MAX_CELLS,
        /// The PSS threshold.
        MinPssDb = MIN_PSS_DB,
        /// Bands left out of the list.
        HideBands = HIDE_BANDS,
        /// Bands the list keeps.
        OnlyBands = ONLY_BANDS,
        /// The least RSRP listed.
        MinRsrp = MIN_RSRP,
        /// List cells with a MIB only.
        MibOnly = MIB_ONLY,
        /// Leave out barred cells.
        NotBarred = NOT_BARRED,
        /// The bands taken first where bands overlap.
        BandOrder = BAND_ORDER,
    }
}

/// The default capture: four SS burst periods of 20 ms.
const CAPTURE_DEFAULT_S: f64 = 0.08;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// Cells searched per candidate.
pub const MAX_CELLS: Setting<field::I64> =
    Setting::new("max_cells", "Cells per candidate", "", 6).range(1.0, 12.0, 1.0);
/// The PSS threshold.
pub const MIN_PSS_DB: Setting<field::F64> =
    Setting::new("min_pss_db", "PSS threshold", "dB", 10.0).range(6.0, 20.0, 0.5);
/// The section of the list filters.
const LIST_SECTION: &str = "List";
/// Bands left out of the list: numbers apart by spaces or commas.
pub const HIDE_BANDS: Setting<field::Text> =
    Setting::new("hide_bands", "Hide bands", "", "").section(LIST_SECTION);
/// Bands the list keeps: numbers apart by spaces or commas. Empty keeps every band.
pub const ONLY_BANDS: Setting<field::Text> =
    Setting::new("only_bands", "Only bands", "", "").section(LIST_SECTION);
/// The least RSRP listed. The lowest value lists every cell.
pub const MIN_RSRP: Setting<field::F64> =
    Setting::new("min_rsrp", "Minimum RSRP", "dBFS", MIN_RSRP_OFF)
        .range(MIN_RSRP_OFF, -20.0, 1.0)
        .section(LIST_SECTION);
/// List cells with a MIB only.
pub const MIB_ONLY: Setting<field::Bool> =
    Setting::new("mib_only", "Only cells with a MIB", "", false).section(LIST_SECTION);
/// Leave out barred cells.
pub const NOT_BARRED: Setting<field::Bool> =
    Setting::new("not_barred", "Leave out barred cells", "", false).section(LIST_SECTION);

/// The settings of the `nr` stack: the search settings and the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The NR receiver: one search component, the stack settings routed to it.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(
        NrSearch::default(),
        MIN_RATE_HZ,
        "5G NR cells needs a sample rate of at least 3.84 Msps",
        cfg,
    )
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the generator of the test signal: the clean signal only, the cells placed against the scene's radio centre.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::from_config(cfg)))
}
