//! TD-SCDMA cells (`tdscdma`): a census of 1.28 Mcps TDD cells by the downlink pilot. For every cell: the SYNC-DL code (the
//! code group), the cell parameter (scrambling code and basic midamble) from the midamble of time slot 0, the frame parity from
//! the phase quadruples of the SYNC-DL, the DwPTS level and SNR and the carrier offset. The broadcast channel is not decoded.
//!
//! Standard: 3GPP TS 25.221, TS 25.223 and TS 25.224 (Annex CA, steps 1 to 3 of the cell search).

pub mod codes;
pub mod gen;
pub mod search;

use crate::decoder::{build_search, capture_setting, Capture, Headline, Log, SearchStack};
use crate::typed::{TdscdmaCellRow, TdscdmaFields, TDSCDMA_CELL, TDSCDMA_CELLS};
use sdrkit_core::interface::{EventKind, Hertz, Time};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    ConfigError, GenConfig, Generator, Receiver, Schema, Setting, SettingError, SettingId,
    StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format, ObjectTable, Seen, Shift};
use search::{TdFound, TdSearcher};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, OnceLock};

/// The stack id.
pub const ID: &str = "tdscdma";
/// The frequency of the test signal, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 2017.4e6;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1.6e6;
/// What the status line shows below [`MIN_RATE_HZ`].
pub const MIN_RATE_MESSAGE: &str = "TD-SCDMA cells needs a sample rate of at least 1.6 Msps";
/// What no public document read for this stack confirms.
pub const NOTE: &str = "The broadcast channel (P-CCPCH) is not decoded. Not confirmed by a public document read for this stack: the root raised cosine pulse with roll-off 0.22 (the setting roll_off), the form of the midamble shifts, the traffic burst of 352 + 144 + 352 + 16 chips, and the complex multipliers of the channelisation codes, which the test signal sends only with gen_multipliers. Multi-frequency cells show no cell on a secondary carrier.";

/// The default capture: 80 ms, four whole phase quadruples.
const CAPTURE_DEFAULT_S: f64 = 0.08;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// The default of [`THRESHOLD`].
const THRESHOLD_DEFAULT: f64 = 6.0;
/// The SYNC-DL threshold: the folded correlation over its median.
pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "SYNC-DL threshold", "", THRESHOLD_DEFAULT)
    .range(3.0, 20.0, 0.5)
    .help("A SYNC-DL code is a candidate when its folded correlation exceeds this many times the median.");
/// The default of [`MAX_CELLS`].
const MAX_CELLS_DEFAULT: i64 = 6;
/// Cells per capture.
pub const MAX_CELLS: Setting<field::I64> = Setting::new("max_cells", "Cells per capture", "", MAX_CELLS_DEFAULT)
    .range(1.0, 16.0, 1.0)
    .help("The most cells measured in one capture, strongest first.");

/// The test signal sends the complex multipliers of the channelisation codes.
pub const GEN_MULTIPLIERS: Setting<field::Bool> = Setting::new("gen_multipliers", "Test signal code multipliers", "", false)
    .help("w(16, 1) = -1 and w(16, 2) = -j on the P-CCPCH; not confirmed against the text of TS 25.223.")
    .section("Test signal")
    .advanced();

/// The default of [`ROLL_OFF`].
const ROLL_OFF_DEFAULT: f64 = gen::ROLL_OFF;
/// The roll-off of the pulse.
pub const ROLL_OFF: Setting<field::F64> = Setting::new("roll_off", "Pulse roll-off", "", ROLL_OFF_DEFAULT)
    .range(0.05, 1.0, 0.01)
    .help("The roll-off of the root raised cosine matched filter (0.22; not confirmed against the text of TS 25.105).")
    .advanced();

sdrkit_core::settings! {
    /// The settings of the TD-SCDMA search component.
    pub enum TdSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// The SYNC-DL threshold.
        Threshold = THRESHOLD,
        /// Cells per capture.
        MaxCells = MAX_CELLS,
        /// The roll-off of the pulse.
        RollOff = ROLL_OFF,
    }
}

/// The most cells the table keeps.
pub const CELL_CAP: usize = 200;

/// The key of a cell: the radio centre (kHz, 0 when unknown) and the SYNC-DL code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TdKey {
    /// The radio centre, kHz.
    pub centre_khz: i64,
    /// The SYNC-DL code.
    pub code: u8,
}

/// "12@2017.400" for code 12 at 2017.4 MHz, "12" without the radio centre.
impl fmt::Display for TdKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.centre_khz == 0 {
            write!(f, "{}", self.code)
        } else {
            write!(f, "{}@{:.3}", self.code, self.centre_khz as f64 / 1e3)
        }
    }
}

/// A cell of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TdCellRow {
    /// The SYNC-DL code.
    pub code: u8,
    /// The initial cell parameter.
    pub param: Option<u8>,
    /// The parameter is sure (else only its pair is known).
    pub sure: bool,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// SNR, dB.
    pub snr_db: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// S1 quadruples counted.
    pub s1: u64,
    /// S2 quadruples counted.
    pub s2: u64,
    /// The SFN parity of the first whole frame of the newest capture.
    pub parity_even: Option<bool>,
    /// Midamble clarity.
    pub clarity: f64,
    /// Captures in which the cell was seen.
    pub captures: u64,
    /// The radio centre of the capture, Hz (0: not known).
    #[serde(default)]
    pub centre_hz: f64,
}

impl Shift for TdCellRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl TdCellRow {
    /// The cell parameter as shown: "37", "36 / 37" for a pair, empty when unknown.
    pub fn parameter_text(&self) -> Option<String> {
        self.param.map(|p| {
            if self.sure {
                p.to_string()
            } else {
                format!("{} / {}", p, p ^ 1)
            }
        })
    }
}

impl Shaped for TdCellRow {
    type Row = TdscdmaCellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        out.put(&TdscdmaCellRow {
            sync_dl: i64::from(self.code),
            parameter: self.parameter_text().as_deref(),
            level_dbfs: self.level_dbfs,
            snr_db: self.snr_db,
            offset_hz: Hertz(self.offset_hz),
            quadruples: &format!("S1 {}, S2 {}", self.s1, self.s2),
            frame_parity: self.parity_even.map(|e| if e { "even" } else { "odd" }),
            clarity: self.clarity,
            captures_seen: self.captures as i64,
            last_heard: columns::last_heard(seen).map(Time),
            frequency: (self.centre_hz > 0.0).then_some(Hertz(self.centre_hz + self.offset_hz)),
            generation: crate::scan::Generation::Third,
            name: &crate::scan::cell_name(None, crate::scan::Generation::Third, Some("TD-SCDMA"), &format!("SYNC-DL {}", self.code)),
        });
    }
}

/// What the stack keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TdTables {
    /// The cells.
    pub cells: ObjectTable<TdKey, TdCellRow>,
}

impl Default for TdTables {
    fn default() -> Self {
        TdTables {
            cells: ObjectTable::new(CELL_CAP),
        }
    }
}

impl Shift for TdTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for TdTables {
    const FORMAT: Format = Format::new("cell.tdscdma_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
    }
}

/// The TD-SCDMA search.
pub struct TdSearch {
    searcher: TdSearcher,
    capture_s: f64,
    threshold: f64,
    max_cells: usize,
}

impl Default for TdSearch {
    fn default() -> Self {
        TdSearch {
            searcher: TdSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            threshold: THRESHOLD_DEFAULT,
            max_cells: MAX_CELLS_DEFAULT as usize,
        }
    }
}

impl SearchStack for TdSearch {
    type Found = Vec<TdFound>;
    type Tables = TdTables;
    type Setting = TdSetting;
    const NAME: &'static str = "cell.tdscdma_search";
    const PREFIX: &'static str = "tdscdma: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.tdscdma_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = TDSCDMA_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &TDSCDMA_CELL;

    fn tables(&self) -> TdTables {
        TdTables::default()
    }
    fn apply(&mut self, s: TdSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match s {
            TdSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            TdSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
            TdSetting::MaxCells => self.max_cells = MAX_CELLS.read(id, v)? as usize,
            TdSetting::RollOff => self.searcher = TdSearcher::new(ROLL_OFF.read(id, v)?),
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (TdSetting::CaptureS, Value::F64(self.capture_s)),
            (TdSetting::Threshold, Value::F64(self.threshold)),
            (TdSetting::MaxCells, Value::I64(self.max_cells as i64)),
            (TdSetting::RollOff, Value::F64(self.searcher.roll_off())),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> Vec<TdFound> {
        self.searcher.search(cap, self.threshold, self.max_cells)
    }
    fn merge(&mut self, t: &mut TdTables, cap: &Capture, found: Vec<TdFound>, log: &mut Log<Self>) {
        let at = cap.end_time;
        let centre_khz = if cap.centre_known() {
            (cap.centre_hz / 1e3).round() as i64
        } else {
            0
        };
        for f in found {
            let key = TdKey {
                centre_khz,
                code: f.code as u8,
            };
            let Ok(up) = t.cells.upsert(&key, log.stamp(at), || TdCellRow {
                code: f.code as u8,
                ..TdCellRow::default()
            }) else {
                continue;
            };
            let row = up.row;
            if up.new {
                log.event(
                    at,
                    key.to_string(),
                    format!("TD-SCDMA: SYNC-DL {} ({:.1} dBFS)", f.code, f.level_dbfs),
                );
            }
            let before = (row.param, row.sure);
            match f.param {
                Some((p, true)) => {
                    row.param = Some(p as u8);
                    row.sure = true;
                }
                Some((p, false)) if !row.sure => row.param = Some(p as u8),
                _ => {}
            }
            if row.sure && (row.param, row.sure) != before {
                if let Some(p) = row.param {
                    *log.decoded += 1;
                    log.event(
                        at,
                        key.to_string(),
                        format!("TD-SCDMA: SYNC-DL {} is cell parameter {p}", f.code),
                    );
                }
            }
            row.level_dbfs = f.level_dbfs;
            row.snr_db = f.snr_db;
            row.offset_hz = f.offset_hz;
            row.centre_hz = if cap.centre_known() { cap.centre_hz } else { 0.0 };
            row.clarity = f.clarity;
            row.s1 += u64::from(f.s1);
            row.s2 += u64::from(f.s2);
            if f.parity_even.is_some() {
                row.parity_even = f.parity_even;
            }
            row.captures += 1;
        }
    }
    fn remove(t: &mut TdTables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut t.cells, row)
    }
    fn publish(&self, t: &TdTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        p.put(&TdscdmaFields { cell_count: n as i64 });
        shaped::rows(&t.cells, &TDSCDMA_CELLS, now, p);
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.level_dbfs.total_cmp(&b.record.level_dbfs))
            .map(|r| r.record.clone());
        Headline {
            summary: format!("TD-SCDMA: {n} cell{}", crate::decoder::plural(n)),
            newest_s: t.cells.newest_s(),
            snr_db: best.as_ref().map(|c| c.snr_db as f32),
            offset_hz: best.map(|c| c.offset_hz),
        }
    }
}

/// The settings of the `tdscdma` stack.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| {
Arc::new(crate::typed::shape(ID).schema())
        })
        .clone()
}

/// The receiver: one search component.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(TdSearch::default(), MIN_RATE_HZ, MIN_RATE_MESSAGE, cfg)
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`build`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the test signal: the cells of [`gen::scene`].
///
/// # Errors
///
/// None; the signature is the descriptor's.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(
        gen::TdGenerator::new(cfg.rate_hz, gen::scene(), cfg.seed, false)
            .with_multipliers(cfg.settings.get(&GEN_MULTIPLIERS))
            .with_roll_off(cfg.settings.get(&ROLL_OFF)),
    ))
}
