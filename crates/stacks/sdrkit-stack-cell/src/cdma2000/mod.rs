//! CDMA2000 cells (`cdma2000`): a census of cdma2000 1x and IS-95 base stations in one 1.25 MHz channel. Every pilot is found
//! by its PN offset and measured (Ec/Io, level, carrier offset, clock error); the Sync Channel of each pilot is decoded and its
//! Sync Channel Message read (SID, NID, PN offset, protocol revision, System Time, Paging Channel). The Paging Channel and
//! everything after it is not decoded. Spreading Rate 1 only.
//!
//! Standard: 3GPP2 C.S0002-C (physical layer), C.S0004 (signalling capsule), C.S0005 (Sync Channel Message).

pub mod gen;
pub mod search;
pub mod sync;

use crate::decoder::{build_search, capture_setting, Capture, Headline, Log, SearchStack};
use crate::typed::{Cdma2000CellRow, Cdma2000Fields, CDMA2000_CELL, CDMA2000_CELLS};
use sdrkit_core::interface::{EventKind, Hertz, Time};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Choice, ConfigError, GenConfig, Generator, Receiver, Schema, Setting, SettingError,
    SettingId, StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_objects::commands::Tables;
use sdrkit_util_objects::publish::columns;
use sdrkit_util_objects::shaped::{self, RowOut, Shaped};
use sdrkit_util_objects::{Format, ObjectTable, Seen, Shift};
use search::{CdmaFound, CdmaSearcher};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::{Arc, OnceLock};
use sync::SyncMessage;

/// The stack id.
pub const ID: &str = "cdma2000";
/// The chip rate of Spreading Rate 1, chips per second.
pub const CHIP_RATE: f64 = 1.2288e6;
/// The frequency of the test signal, Hz. The host gives the frequency of a real channel.
pub const TEST_SIGNAL_HZ: f64 = 878.49e6;
/// The least input rate, Hz.
pub const MIN_RATE_HZ: f64 = 1.6e6;
/// What the status line shows below [`MIN_RATE_HZ`].
pub const MIN_RATE_MESSAGE: &str = "CDMA2000 cells needs a sample rate of at least 1.6 Msps";
/// What the stack does not read, and what no public document confirms.
pub const NOTE: &str = "The Paging Channel and everything after it are not decoded; Spreading Rate 1 only. The matched filter at two samples a chip is derived from the 48-tap baseband filter of C.S0002-C (adjacent taps added); the standard gives the filter only for the transmitter. The decode is verified on generated signals only.";

/// The coefficients h(0) to h(23) of the baseband filter at 4 samples a chip (C.S0002-C 3.1.3.1.17.1, table
/// 3.1.3.1.17.1-1); h(k) = h(47 - k).
const FIR_HALF: [f64; 24] = [
    -0.025288315,
    -0.034167931,
    -0.035752323,
    -0.016733702,
    0.021602514,
    0.064938487,
    0.091002137,
    0.081894974,
    0.037071157,
    -0.021998074,
    -0.060716277,
    -0.051178658,
    0.007874526,
    0.084368728,
    0.126869306,
    0.094528345,
    -0.012839661,
    -0.143477028,
    -0.211829088,
    -0.140513128,
    0.094601918,
    0.441387140,
    0.785875640,
    1.0,
];

/// The 48 taps of the baseband filter at 4 samples a chip.
pub fn baseband_fir() -> [f64; 48] {
    let mut h = [0.0; 48];
    for k in 0..24 {
        h[k] = FIR_HALF[k];
        h[47 - k] = FIR_HALF[k];
    }
    h
}

/// The matched filter at 2 samples a chip derived from the baseband filter: `g(k) = h(2k) + h(2k + 1)`, 24 taps, unit energy.
pub fn matched_taps() -> Vec<f32> {
    let h = baseband_fir();
    let g: Vec<f64> = (0..24).map(|k| h[2 * k] + h[2 * k + 1]).collect();
    let e = g.iter().map(|v| v * v).sum::<f64>().sqrt();
    g.iter().map(|v| (v / e) as f32).collect()
}

sdrkit_core::choice_enum! {
    /// The band class of the channel, for the Paging Channel frequency.
    pub enum BandClass {
        /// Not known.
        None = ("none", "Not known"),
        /// Band class 0, 800 MHz.
        Bc0 = ("0", "0 (800 MHz)"),
        /// Band class 1, 1900 MHz.
        Bc1 = ("1", "1 (1900 MHz)"),
        /// Band class 4, 1800 MHz.
        Bc4 = ("4", "4 (1800 MHz)"),
    }
}

impl BandClass {
    /// The band class number.
    pub fn number(self) -> Option<u8> {
        match self {
            BandClass::None => None,
            BandClass::Bc0 => Some(0),
            BandClass::Bc1 => Some(1),
            BandClass::Bc4 => Some(4),
        }
    }
}

/// The default capture: 36 whole PN periods and a little.
const CAPTURE_DEFAULT_S: f64 = 0.98;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// The band class.
pub const BAND_CLASS: Setting<field::Choice<BandClass>> = Setting::choice("band_class", "Band class", BandClass::None)
    .help("The band class of the channel; the host sets it from its data. Only the Paging Channel frequency uses it.");
/// The default of [`MAX_OFFSET_HZ`].
const MAX_OFFSET_DEFAULT: f64 = 5000.0;
/// The carrier search range.
pub const MAX_OFFSET_HZ: Setting<field::F64> = Setting::new("max_offset_hz", "Carrier search range", "Hz", MAX_OFFSET_DEFAULT)
    .range(500.0, 10_000.0, 100.0)
    .help("Pilots are searched within this carrier offset either side.");
/// The default of [`MAX_CELLS`].
const MAX_CELLS_DEFAULT: i64 = 6;
/// Pilots per capture.
pub const MAX_CELLS: Setting<field::I64> = Setting::new("max_cells", "Pilots per capture", "", MAX_CELLS_DEFAULT)
    .range(1.0, 16.0, 1.0)
    .help("The most pilots measured in one capture, strongest first.");
/// The default of [`THRESHOLD`].
const THRESHOLD_DEFAULT: f64 = 2.4;
/// The pilot threshold.
pub const THRESHOLD: Setting<field::F64> = Setting::new("threshold", "Pilot threshold", "", THRESHOLD_DEFAULT)
    .range(1.5, 6.0, 0.1)
    .help("A code phase is a candidate when its power exceeds this many times the median.");

sdrkit_core::settings! {
    /// The settings of the cdma2000 search component.
    pub enum CdmaSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// The band class.
        BandClass = BAND_CLASS,
        /// The carrier search range.
        MaxOffsetHz = MAX_OFFSET_HZ,
        /// Pilots per capture.
        MaxCells = MAX_CELLS,
        /// The pilot threshold.
        Threshold = THRESHOLD,
    }
}

/// The most rows the table keeps.
pub const CELL_CAP: usize = 200;

/// The key of a base station: the radio centre (kHz, 0 when unknown) and its PN offset, or its offset relative to the
/// strongest pilot when it has no message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CdmaKey {
    /// A pilot with a message.
    Pn {
        /// Radio centre, kHz.
        centre_khz: i64,
        /// PILOT_PN.
        pn: u16,
    },
    /// A pilot without a message.
    Relative {
        /// Radio centre, kHz.
        centre_khz: i64,
        /// Offset from the strongest pilot, units of 64 chips.
        rel: u16,
    },
}

/// "168@878.490" or "+5@878.490" ("168", "+5" without the radio centre).
impl fmt::Display for CdmaKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (text, c) = match *self {
            CdmaKey::Pn { centre_khz, pn } => (pn.to_string(), centre_khz),
            CdmaKey::Relative { centre_khz, rel } => (format!("+{rel}"), centre_khz),
        };
        if c == 0 {
            write!(f, "{text}")
        } else {
            write!(f, "{text}@{:.3}", c as f64 / 1e3)
        }
    }
}

/// A base station of the table.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CdmaRow {
    /// Offset from the strongest pilot of the newest capture, units of 64 chips.
    pub relative: u16,
    /// The PN offset derived from another pilot.
    pub derived_pn: Option<u16>,
    /// Ec/Io, dB.
    pub ec_io_db: f64,
    /// Level, dBFS.
    pub level_dbfs: f64,
    /// Carrier offset, Hz.
    pub offset_hz: f64,
    /// Clock error, ppm.
    pub clock_ppm: f64,
    /// The newest good message.
    pub msg: Option<SyncMessage>,
    /// The band class known when the message was read.
    pub band_class: Option<u8>,
    /// Good messages.
    pub good: u64,
    /// CRC failures.
    pub bad: u64,
    /// Captures in which the pilot was seen.
    pub captures: u64,
    /// The radio centre of the capture, Hz (0: not known).
    #[serde(default)]
    pub centre_hz: f64,
}

impl Shift for CdmaRow {
    fn shift_times(&mut self, _by_s: f64) {}
}

impl CdmaRow {
    /// "9600 bps, ch 283 (878.490 MHz)", with the MHz when the band class is known.
    pub fn paging_text(&self) -> Option<String> {
        let m = self.msg.as_ref()?;
        let rate = match m.prat {
            0 => "9600 bps",
            1 => "4800 bps",
            _ => "reserved rate",
        };
        let mhz = self
            .band_class
            .and_then(|bc| sync::channel_mhz(bc, m.cdma_freq))
            .map(|f| format!(" ({f:.3} MHz)"))
            .unwrap_or_default();
        let ext = if m.ext_cdma_freq != m.cdma_freq {
            format!(", extended ch {}", m.ext_cdma_freq)
        } else {
            String::new()
        };
        Some(format!("{rate}, ch {}{mhz}{ext}", m.cdma_freq))
    }

    fn pn_text(&self) -> String {
        match (&self.msg, self.derived_pn) {
            (Some(m), _) => m.pilot_pn.to_string(),
            (None, Some(p)) => format!("PN {p} (derived)"),
            (None, None) => format!("+{}", self.relative),
        }
    }
}

impl Shaped for CdmaRow {
    type Row = Cdma2000CellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let m = self.msg.as_ref();
        let local = m.map(|m| {
            format!(
                "{}, UTC{:+.1} h{}, {} leap seconds",
                m.local_text(),
                f64::from(m.ltm_off) / 2.0,
                if m.daylt { ", daylight saving" } else { "" },
                m.lp_sec
            )
        });
        out.put(&Cdma2000CellRow {
            pn: &self.pn_text(),
            sid: m.map(|m| i64::from(m.sid)),
            nid: m.map(|m| i64::from(m.nid)),
            ec_io_db: self.ec_io_db,
            level_dbfs: self.level_dbfs,
            offset_hz: Hertz(self.offset_hz),
            clock_ppm: self.clock_ppm,
            paging: self.paging_text().as_deref(),
            protocol: m.map(|m| format!("P_REV {} (min {})", m.p_rev, m.min_p_rev)).as_deref(),
            system_time: m.map(SyncMessage::utc_text).as_deref(),
            local_time: local.as_deref(),
            lc_state: m.map(|m| format!("{:011X}", m.lc_state)).as_deref(),
            messages: &format!("{} / {}", self.good, self.good + self.bad),
            last_heard: columns::last_heard(seen).map(Time),
            frequency: (self.centre_hz > 0.0).then_some(Hertz(self.centre_hz + self.offset_hz)),
            generation: crate::scan::Generation::Third,
            name: &crate::scan::cell_name(
                m.map(|m| format!("SID {}", m.sid)).as_deref(),
                crate::scan::Generation::Third,
                Some("CDMA2000"),
                &format!("PN {}", self.pn_text()),
            ),
        });
    }
}

/// What the stack keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CdmaTables {
    /// The base stations.
    pub cells: ObjectTable<CdmaKey, CdmaRow>,
}

impl Default for CdmaTables {
    fn default() -> Self {
        CdmaTables {
            cells: ObjectTable::new(CELL_CAP),
        }
    }
}

impl Shift for CdmaTables {
    fn shift_times(&mut self, by_s: f64) {
        self.cells.shift_times(by_s);
    }
}

impl Tables for CdmaTables {
    const FORMAT: Format = Format::new("cell.cdma2000_cells", 1);
    fn clear(&mut self) {
        self.cells.clear();
    }
    fn adopt(&mut self, saved: Self) {
        self.cells.adopt(saved.cells);
    }
}

/// The cdma2000 search.
pub struct CdmaSearch {
    searcher: CdmaSearcher,
    capture_s: f64,
    band_class: BandClass,
    max_offset_hz: f64,
    max_cells: usize,
    threshold: f64,
}

impl Default for CdmaSearch {
    fn default() -> Self {
        CdmaSearch {
            searcher: CdmaSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            band_class: BandClass::None,
            max_offset_hz: MAX_OFFSET_DEFAULT,
            max_cells: MAX_CELLS_DEFAULT as usize,
            threshold: THRESHOLD_DEFAULT,
        }
    }
}

impl SearchStack for CdmaSearch {
    type Found = Vec<CdmaFound>;
    type Tables = CdmaTables;
    type Setting = CdmaSetting;
    const NAME: &'static str = "cell.cdma_search";
    const PREFIX: &'static str = "cdma2000: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.cdma2000_cell";
    const RECENT_S: f64 = 10.0;
    const CELLS: &'static TableDef = CDMA2000_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &CDMA2000_CELL;

    fn tables(&self) -> CdmaTables {
        CdmaTables::default()
    }
    fn apply(&mut self, s: CdmaSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match s {
            CdmaSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            CdmaSetting::BandClass => self.band_class = BAND_CLASS.read(id, v)?,
            CdmaSetting::MaxOffsetHz => self.max_offset_hz = MAX_OFFSET_HZ.read(id, v)?,
            CdmaSetting::MaxCells => self.max_cells = MAX_CELLS.read(id, v)? as usize,
            CdmaSetting::Threshold => self.threshold = THRESHOLD.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (CdmaSetting::CaptureS, Value::F64(self.capture_s)),
            (CdmaSetting::BandClass, Value::Choice(self.band_class.index())),
            (CdmaSetting::MaxOffsetHz, Value::F64(self.max_offset_hz)),
            (CdmaSetting::MaxCells, Value::I64(self.max_cells as i64)),
            (CdmaSetting::Threshold, Value::F64(self.threshold)),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> Vec<CdmaFound> {
        self.searcher
            .search(cap, self.max_offset_hz, self.max_cells, self.threshold)
    }
    fn merge(&mut self, t: &mut CdmaTables, cap: &Capture, found: Vec<CdmaFound>, log: &mut Log<Self>) {
        let at = cap.end_time;
        let centre_khz = if cap.centre_known() {
            (cap.centre_hz / 1e3).round() as i64
        } else {
            0
        };
        // Rows without a message that a pilot with a message of this capture now names are dropped.
        for f in found.iter().filter(|f| f.msg.is_some()) {
            let stale = CdmaKey::Relative {
                centre_khz,
                rel: f.relative,
            };
            sdrkit_util_objects::rows::remove_row(&mut t.cells, &stale.to_string());
        }
        for f in found {
            let key = match &f.msg {
                Some(m) => CdmaKey::Pn {
                    centre_khz,
                    pn: m.pilot_pn,
                },
                None => CdmaKey::Relative {
                    centre_khz,
                    rel: f.relative,
                },
            };
            let Ok(up) = t.cells.upsert(&key, log.stamp(at), CdmaRow::default) else {
                continue;
            };
            let row = up.row;
            if up.new {
                let text = match &f.msg {
                    Some(m) => format!(
                        "CDMA2000: PN {}, SID {}, NID {} (Ec/Io {:.1} dB)",
                        m.pilot_pn, m.sid, m.nid, f.ec_io_db
                    ),
                    None => format!(
                        "CDMA2000: pilot at +{} x 64 chips (Ec/Io {:.1} dB)",
                        f.relative, f.ec_io_db
                    ),
                };
                log.event(at, key.to_string(), text);
            }
            row.relative = f.relative;
            row.derived_pn = f.derived_pn;
            row.ec_io_db = f.ec_io_db;
            row.level_dbfs = f.level_dbfs;
            row.offset_hz = f.offset_hz;
            row.centre_hz = if cap.centre_known() { cap.centre_hz } else { 0.0 };
            row.clock_ppm = f.clock_ppm;
            row.captures += 1;
            row.good += u64::from(f.good);
            row.bad += u64::from(f.bad);
            *log.decoded += u64::from(f.good);
            *log.failed += u64::from(f.bad);
            if let Some(m) = f.msg {
                row.msg = Some(m);
                row.band_class = self.band_class.number();
            }
        }
    }
    fn remove(t: &mut CdmaTables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut t.cells, row)
    }
    fn publish(&self, t: &CdmaTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        let identified = t.cells.iter().filter(|r| r.record.msg.is_some()).count();
        p.put(&Cdma2000Fields {
            station_count: n as i64,
            identified: identified as i64,
        });
        shaped::rows(&t.cells, &CDMA2000_CELLS, now, p);
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.ec_io_db.total_cmp(&b.record.ec_io_db))
            .map(|r| (r.record.ec_io_db, r.record.offset_hz));
        let band = self
            .band_class
            .number()
            .map(|b| format!(", band class {b}"))
            .unwrap_or_default();
        Headline {
            summary: format!(
                "CDMA2000: {n} base station{}, {identified} identified{band}",
                crate::decoder::plural(n)
            ),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|b| b.0 as f32),
            offset_hz: best.map(|b| b.1),
        }
    }
}

/// The settings of the `cdma2000` stack.
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
    build_search(CdmaSearch::default(), MIN_RATE_HZ, MIN_RATE_MESSAGE, cfg)
}

/// Makes the receiver.
///
/// # Errors
///
/// As [`build`].
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the test signal: the stations of [`gen::scene`] from system chip [`gen::SCENE_START_CHIP`], with the baseband filter.
///
/// # Errors
///
/// None; the signature is the descriptor's.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(gen::CdmaGenerator::new(
        cfg.rate_hz,
        gen::scene(),
        gen::SCENE_START_CHIP,
        gen::Pulse::Fir,
        cfg.seed,
    )))
}
