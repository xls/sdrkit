//! UMTS cells (`umts`): a census of WCDMA FDD cells in one 5 MHz channel. For every cell: the primary scrambling code, the code
//! group, Ec/Io, level, carrier offset and frame timing from the synchronisation channels and the CPICH, and from the broadcast
//! channel the PLMN (MIB) and the cell identity (SIB 3). Nothing else of the broadcast is read.
//!
//! Standard: 3GPP TS 25.211, TS 25.212, TS 25.213, TS 25.331.

pub mod bch;
pub mod gen;
pub mod search;

use crate::decoder::{build_search, capture_setting, Capture, Headline, Log, SearchStack};
use crate::typed::{UmtsCellRow, UmtsFields, UMTS_CELL, UMTS_CELLS};
use sdrkit_core::interface::{EventKind, Hertz, Time};
use bch::Mib;
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, GenConfig, Generator, Receiver, Schema, Setting, SettingError, SettingId,
    StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_ids::mcc::{self, Plmn};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format, ObjectTable, Seen, Shift};
use search::{UmtsFound, UmtsSearcher};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "umts";
/// The chip rate, chips per second.
pub const CHIP_RATE: f64 = 3.84e6;
/// The frequency of the test signal, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 2112.8e6;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 3.84e6;
/// What the status line shows below [`MIN_RATE_HZ`].
pub const MIN_RATE_MESSAGE: &str = "UMTS cells needs a sample rate of at least 3.84 Msps";
/// What the stack does not read, and what no public document read for it confirms.
pub const NOTE: &str = "Only the MIB (PLMN) and SIB 3 (cell identity) of the broadcast channel are read; an STTD-encoded P-CCPCH is not combined. The decode is verified on generated signals only. Not confirmed against the text of a public document: the BCH transport block of 246 bits, the root raised cosine pulse of roll-off 0.22 (the setting roll_off), the split of the cell identity into 12 bits of RNC and 16 bits of cell, and the channel raster UARFCN = 5 x f (MHz).";

/// The default capture: 8 frames.
const CAPTURE_DEFAULT_S: f64 = 0.08;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// The default of [`MAX_OFFSET_HZ`].
const MAX_OFFSET_DEFAULT: f64 = 7500.0;
/// The carrier search range.
pub const MAX_OFFSET_HZ: Setting<field::F64> = Setting::new("max_offset_hz", "Carrier search range", "Hz", MAX_OFFSET_DEFAULT)
    .range(2500.0, 12_500.0, 2500.0)
    .help("Cells are searched within this carrier offset either side, in steps of 2.5 kHz.");
/// The default of [`MAX_CELLS`].
const MAX_CELLS_DEFAULT: i64 = 8;
/// Cells per capture.
pub const MAX_CELLS: Setting<field::I64> = Setting::new("max_cells", "Cells per capture", "", MAX_CELLS_DEFAULT)
    .range(1.0, 16.0, 1.0)
    .help("The most cells measured in one capture, strongest first.");
/// Read the broadcast channel.
pub const READ_BCH: Setting<field::Bool> = Setting::new("read_bch", "Read the broadcast channel", "", true)
    .help("Decode the BCH of each cell for its PLMN and cell identity.");
/// The default of [`ROLL_OFF`].
const ROLL_OFF_DEFAULT: f64 = 0.22;
/// The roll-off of the matched filter.
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "Pulse roll-off", "", ROLL_OFF_DEFAULT)
    .range(0.05, 1.0, 0.01)
    .help("The roll-off of the root raised cosine matched filter (0.22 by TS 25.104; not confirmed against the text).")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the UMTS search component.
    pub enum UmtsSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// The carrier search range.
        MaxOffsetHz = MAX_OFFSET_HZ,
        /// Cells per capture.
        MaxCells = MAX_CELLS,
        /// Read the broadcast channel.
        ReadBch = READ_BCH,
        /// The roll-off of the matched filter.
        RollOff = ROLL_OFF,
    }
}

/// The most cells the table keeps.
pub const CELL_CAP: usize = 400;

/// The key of a cell: the radio centre (kHz, 0 when unknown) and the primary scrambling code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UmtsKey {
    /// Radio centre, kHz.
    pub centre_khz: i64,
    /// Primary scrambling code.
    pub code: u16,
}

/// "17@2112.800", or "17" without the radio centre.
impl fmt::Display for UmtsKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.centre_khz == 0 {
            write!(f, "{}", self.code)
        } else {
            write!(f, "{}@{:.3}", self.code, self.centre_khz as f64 / 1e3)
        }
    }
}

/// The PLMN of a cell as kept.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlmnRow {
    /// MCC.
    pub mcc: u16,
    /// MNC.
    pub mnc: u16,
    /// Digits of the MNC.
    pub mnc_digits: u8,
}

impl PlmnRow {
    fn plmn(&self) -> Plmn {
        Plmn {
            mcc: self.mcc,
            mnc: self.mnc,
            mnc_digits: self.mnc_digits,
        }
    }
}

/// A cell of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UmtsRow {
    /// Primary scrambling code.
    pub code: u16,
    /// Code group.
    pub group: u8,
    /// Ec/Io, dB.
    pub ec_io_db: f64,
    /// CPICH level, dBFS.
    pub rscp_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Frame start in the newest capture, chips.
    pub frame_offset: u32,
    /// The SCH symbol a is +1.
    pub sttd: Option<bool>,
    /// Captures in which the cell was seen.
    pub captures: u64,
    /// Transport blocks with a right CRC.
    pub bch_blocks: u64,
    /// The PLMN of the MIB.
    pub plmn: Option<PlmnRow>,
    /// The cell identity of SIB 3.
    pub cell_id: Option<u32>,
    /// The channel number of the radio centre.
    pub uarfcn: Option<u32>,
}

impl Shift for UmtsRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl Shaped for UmtsRow {
    type Row = UmtsCellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let plmn = self.plmn.as_ref().map(PlmnRow::plmn);
        out.put(&UmtsCellRow {
            code: i64::from(self.code),
            group: i64::from(self.group),
            ec_io_db: self.ec_io_db,
            level_dbfs: self.rscp_dbfs,
            offset_hz: Hertz(self.offset_hz),
            frame_offset: i64::from(self.frame_offset),
            sttd: self.sttd,
            captures_seen: self.captures as i64,
            bch_blocks: self.bch_blocks as i64,
            plmn: plmn.map(|p| p.to_string()).as_deref(),
            operator: plmn.and_then(mcc::operator),
            cell_id: self.cell_id.map(i64::from),
            cell_id_parts: self.cell_id.map(|c| format!("RNC {}, cell {}", c >> 16, c & 0xFFFF)).as_deref(),
            uarfcn: self.uarfcn.map(i64::from),
            last_heard: columns::last_heard(seen).map(Time),
        });
    }
}

/// What the stack keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UmtsTables {
    /// The cells.
    pub cells: ObjectTable<UmtsKey, UmtsRow>,
}

impl Default for UmtsTables {
    fn default() -> Self {
        UmtsTables {
            cells: ObjectTable::new(CELL_CAP),
        }
    }
}

impl Shift for UmtsTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for UmtsTables {
    const FORMAT: Format = Format::new("cell.umts_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
    }
}

/// The UMTS search.
pub struct UmtsSearch {
    searcher: UmtsSearcher,
    capture_s: f64,
    max_offset_hz: f64,
    max_cells: usize,
    read_bch: bool,
}

impl Default for UmtsSearch {
    fn default() -> Self {
        UmtsSearch {
            searcher: UmtsSearcher::new(ROLL_OFF_DEFAULT),
            capture_s: CAPTURE_DEFAULT_S,
            max_offset_hz: MAX_OFFSET_DEFAULT,
            max_cells: MAX_CELLS_DEFAULT as usize,
            read_bch: true,
        }
    }
}

/// "246-03, cell 11259375", the text of what a row's broadcast names.
fn identity_text(row: &UmtsRow) -> String {
    let mut parts = Vec::new();
    if let Some(p) = &row.plmn {
        parts.push(p.plmn().to_string());
    }
    if let Some(c) = row.cell_id {
        parts.push(format!("cell {c}"));
    }
    parts.join(", ")
}

impl SearchStack for UmtsSearch {
    type Found = Vec<UmtsFound>;
    type Tables = UmtsTables;
    type Setting = UmtsSetting;
    const NAME: &'static str = "cell.umts_search";
    const PREFIX: &'static str = "umts: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.umts_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = UMTS_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &UMTS_CELL;

    fn tables(&self) -> UmtsTables {
        UmtsTables::default()
    }
    fn apply(&mut self, s: UmtsSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match s {
            UmtsSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            UmtsSetting::MaxOffsetHz => self.max_offset_hz = MAX_OFFSET_HZ.read(id, v)?,
            UmtsSetting::MaxCells => self.max_cells = MAX_CELLS.read(id, v)? as usize,
            UmtsSetting::ReadBch => self.read_bch = READ_BCH.read(id, v)?,
            UmtsSetting::RollOff => self.searcher = UmtsSearcher::new(ROLL_OFF.read(id, v)?),
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (UmtsSetting::CaptureS, Value::F64(self.capture_s)),
            (UmtsSetting::MaxOffsetHz, Value::F64(self.max_offset_hz)),
            (UmtsSetting::MaxCells, Value::I64(self.max_cells as i64)),
            (UmtsSetting::ReadBch, Value::Bool(self.read_bch)),
            (UmtsSetting::RollOff, Value::F64(self.searcher.roll_off())),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> Vec<UmtsFound> {
        self.searcher
            .search(cap, self.max_offset_hz, self.max_cells, self.read_bch)
    }
    fn merge(&mut self, t: &mut UmtsTables, cap: &Capture, found: Vec<UmtsFound>, log: &mut Log<Self>) {
        let at = cap.end_time;
        let known = cap.centre_known();
        let centre_khz = if known { (cap.centre_hz / 1e3).round() as i64 } else { 0 };
        for f in found {
            let key = UmtsKey {
                centre_khz,
                code: f.code as u16,
            };
            let Ok(up) = t.cells.upsert(&key, log.stamp(at), || UmtsRow {
                code: f.code as u16,
                group: f.group as u8,
                uarfcn: known.then(|| (5.0 * cap.centre_hz / 1e6).round() as u32),
                ..UmtsRow::default()
            }) else {
                continue;
            };
            let row = up.row;
            if up.new {
                log.event(
                    at,
                    key.to_string(),
                    format!("UMTS: cell PSC {} (Ec/Io {:.1} dB)", f.code, f.ec_io_db),
                );
            }
            let before = (row.plmn.clone(), row.cell_id);
            row.ec_io_db = f.ec_io_db;
            row.rscp_dbfs = f.rscp_dbfs;
            row.offset_hz = f.offset_hz;
            row.frame_offset = f.frame_offset as u32;
            row.sttd = f.sttd;
            row.captures += 1;
            row.bch_blocks += u64::from(f.good_blocks);
            *log.decoded += u64::from(f.good_blocks);
            if let Some(m) = f.mib {
                row.plmn = Some(PlmnRow {
                    mcc: m.mcc,
                    mnc: m.mnc,
                    mnc_digits: m.mnc_digits,
                });
            }
            if f.cell_id.is_some() {
                row.cell_id = f.cell_id;
            }
            let first_plmn = before.0.is_none() && row.plmn.is_some();
            let first_cell = before.1.is_none() && row.cell_id.is_some();
            if first_plmn || first_cell {
                log.event(
                    at,
                    key.to_string(),
                    format!("UMTS: PSC {} is {}", f.code, identity_text(row)),
                );
            }
        }
    }
    fn remove(t: &mut UmtsTables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut t.cells, row)
    }
    fn publish(&self, t: &UmtsTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        p.put(&UmtsFields { cell_count: n as i64 });
        shaped::rows(&t.cells, &UMTS_CELLS, now, p);
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.ec_io_db.total_cmp(&b.record.ec_io_db))
            .map(|r| (r.record.ec_io_db, r.record.offset_hz));
        Headline {
            summary: format!("UMTS: {n} cell{}", crate::decoder::plural(n)),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|b| b.0 as f32),
            offset_hz: best.map(|b| b.1),
        }
    }
}

/// The settings of the `umts` stack.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The receiver: one search component.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(UmtsSearch::default(), MIN_RATE_HZ, MIN_RATE_MESSAGE, cfg)
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`build`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the test signal: the cells of [`gen::scene`] with the pulse of the roll-off setting.
///
/// # Errors
///
/// None; the signature is the descriptor's.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let roll_off = cfg.settings.get(&ROLL_OFF);
    Ok(Box::new(gen::UmtsGenerator::new(cfg.rate_hz, gen::scene(), cfg.seed, roll_off)))
}

/// The PLMN of a MIB as kept.
impl From<Mib> for PlmnRow {
    fn from(m: Mib) -> Self {
        PlmnRow {
            mcc: m.mcc,
            mnc: m.mnc,
            mnc_digits: m.mnc_digits,
        }
    }
}
