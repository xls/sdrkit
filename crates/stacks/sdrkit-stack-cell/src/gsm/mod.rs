//! The GSM and GSM-R base stations stack (`gsmr`): FCCH, SCH and the BCCH System Information. Paging, access grants,
//! traffic channels and the uplink are never decoded.
//!
//! Standard: 3GPP TS 45.002, TS 45.005 and TS 44.018.

pub mod codes;
pub mod gen;
pub mod gmsk;
pub mod search;
pub mod si;
pub mod table;

use crate::decoder::{build_search, capture_setting, Capture, Headline, Log, SearchStack};
use crate::typed::{GsmrCarrierRow, GsmrFields, GSMR_CARRIERS, GSMR_CELL, GSMR_CELLS};
use sdrkit_core::interface::{EventKind, Hertz};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::{
    Choice, ConfigError, DeclareSetting, GenConfig, Generator, Receiver, Schema, Setting,
    SettingError, SettingId, StackConfig, Value, Values,
};
use sdrkit_graph::{values_of, GraphError, StackGraph};
use sdrkit_util_objects::shaped;
use search::{GsmSearchOut, GsmSearcher};
use std::sync::{Arc, OnceLock};
use table::GsmTables;

/// The stack id.
pub const ID: &str = "gsmr";
/// The lowest input rate, Hz: a 200 kHz carrier at 4 samples per symbol after the channeliser needs about 1 Msps.
pub const MIN_RATE_HZ: f64 = 1e6;
/// What the status line shows below [`MIN_RATE_HZ`].
pub const MIN_RATE_MESSAGE: &str = "GSM / GSM-R needs a sample rate of at least 1 Msps";

sdrkit_core::choice_enum! {
    /// The channel rasters searched.
    pub enum BandRaster {
        /// The GSM 900 raster only.
        Gsm900 = ("900", "900 MHz"),
        /// The 900, 1800 and 850 MHz rasters.
        All = ("all", "900 + 1800 + 850"),
    }
}

sdrkit_core::choice_enum! {
    /// The carriers of the test signal.
    pub enum GsmScene {
        /// Two cells.
        TwoCells = ("0", "Two cells"),
        /// A cell at the band edge.
        BandEdge = ("1", "Band edge"),
        /// Three cells.
        ThreeCells = ("2", "Three cells"),
    }
}

sdrkit_core::settings! {
    /// The settings of the GSM search component.
    pub enum GsmSetting {
        /// Seconds of input searched at a time.
        CaptureS = CAPTURE_S,
        /// Carriers searched per capture.
        MaxCarriers = MAX_CARRIERS,
        /// The channel rasters searched.
        Band = GSM_BAND,
    }
}

/// The default capture: a whole cycle of the eight System Information positions of the BCCH (1.88 s), with room.
const CAPTURE_DEFAULT_S: f64 = 2.2;
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// The default of [`MAX_CARRIERS`].
const MAX_CARRIERS_DEFAULT: i64 = 12;
/// Carriers searched per capture, strongest first.
pub const MAX_CARRIERS: Setting<field::I64> = Setting::new(
    "max_carriers",
    "Carriers per capture",
    "",
    MAX_CARRIERS_DEFAULT,
)
.range(1.0, 24.0, 1.0)
.help("Humps searched, strongest first.");
/// The channel rasters searched.
pub const GSM_BAND: Setting<field::Choice<BandRaster>> =
    Setting::choice("gsm_band", "Band raster", BandRaster::Gsm900);
/// The section of the test signal settings.
const TEST_SIGNAL: &str = "Test signal";
/// The carriers of the test signal.
pub const GEN_SCENE: Setting<field::Choice<GsmScene>> =
    Setting::choice("gen_scene", "Test signal scene", GsmScene::TwoCells)
        .section(TEST_SIGNAL)
        .advanced();
/// The radio centre that the test carriers are placed against.
pub const GEN_CENTRE_HZ: Setting<field::Hz> = Setting::new(
    "gen_centre_hz",
    "Test signal radio centre",
    "Hz",
    gen::CENTRE_HZ,
)
.range(860e6, 960e6, 1e3)
.help("The radio centre the test cells are placed against.")
.section(TEST_SIGNAL)
.advanced();
/// The test signal settings of the stack.
pub const GEN_SETTINGS: &[&dyn DeclareSetting] = &[&GEN_SCENE, &GEN_CENTRE_HZ];

/// The GSM search: what a capture holds and how the cells are kept.
pub struct GsmSearch {
    searcher: GsmSearcher,
    capture_s: f64,
    max_carriers: usize,
    band: BandRaster,
}

impl Default for GsmSearch {
    fn default() -> Self {
        GsmSearch {
            searcher: GsmSearcher::default(),
            capture_s: CAPTURE_DEFAULT_S,
            max_carriers: MAX_CARRIERS_DEFAULT as usize,
            band: BandRaster::Gsm900,
        }
    }
}

impl SearchStack for GsmSearch {
    type Found = GsmSearchOut;
    type Tables = GsmTables;
    type Setting = GsmSetting;
    const NAME: &'static str = "cell.gsm_search";
    const PREFIX: &'static str = "gsmr: ";
    const ID: &'static str = ID;
    const EVENT_KIND: &'static str = "cell.gsm_cell";
    const RECENT_S: f64 = 30.0;
    const CELLS: &'static TableDef = GSMR_CELLS.def();
    const EVENT: &'static EventKind<crate::typed::FoundCellEvent<'static>> = &GSMR_CELL;

    fn tables(&self) -> GsmTables {
        GsmTables::default()
    }
    fn apply(&mut self, s: GsmSetting, id: SettingId, v: &Value) -> Result<(), SettingError> {
        match s {
            GsmSetting::CaptureS => self.capture_s = CAPTURE_S.read(id, v)?,
            GsmSetting::MaxCarriers => self.max_carriers = MAX_CARRIERS.read(id, v)? as usize,
            GsmSetting::Band => self.band = GSM_BAND.read(id, v)?,
        }
        Ok(())
    }
    fn values(&self) -> Values {
        values_of([
            (GsmSetting::CaptureS, Value::F64(self.capture_s)),
            (
                GsmSetting::MaxCarriers,
                Value::I64(self.max_carriers as i64),
            ),
            (GsmSetting::Band, Value::Choice(self.band.index())),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> GsmSearchOut {
        self.searcher.search(cap, self.max_carriers)
    }
    fn merge(
        &mut self,
        tables: &mut GsmTables,
        cap: &Capture,
        found: GsmSearchOut,
        log: &mut Log<Self>,
    ) {
        tables.merge(cap, found, self.band == BandRaster::All, log);
    }
    fn remove(tables: &mut Self::Tables, row: &str) -> bool {
        sdrkit_util_objects::rows::remove_row(&mut tables.cells, row)
    }
    fn forget_capture(tables: &mut GsmTables) {
        tables.carriers.clear();
    }
    fn publish(&self, t: &GsmTables, now: f64, p: &mut Published) -> Headline {
        let n = t.cells.len();
        let identified = t.cells.iter().filter(|r| r.record.lai.is_some()).count();
        let railway = t.cells.iter().filter(|r| r.record.railway_band).count();
        p.put(&GsmrFields {
            cell_count: n as i64,
            identified: identified as i64,
            railway_cells: railway as i64,
        });
        shaped::rows(&t.cells, &GSMR_CELLS, now, p);
        let survey = t.cells.iter().map(|r| r.record.survey(&r.key.to_string())).collect();
        crate::scan::write_survey(&crate::typed::GSMR_SURVEY, survey, p);
        shaped::indexed_rows(p, &GSMR_CARRIERS, &t.carriers, |c, out| {
            out.put(&GsmrCarrierRow {
                frequency: Hertz(c.frequency_hz),
                level_db: c.level_db,
                synchronised: c.synchronised,
            });
        });
        // The strongest cell by level among those seen within the recent window.
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.level_dbfs.total_cmp(&b.record.level_dbfs))
            .map(|r| r.record);
        Headline {
            summary: format!("{n} cells, {identified} identified, {railway} in the railway band"),
            newest_s: t.cells.newest_s(),
            snr_db: best.map(|c| c.snr_db as f32),
            offset_hz: best.map(|c| c.carrier_offset_hz),
        }
    }
}

/// The settings of the `gsmr` stack: the search settings and the test signal settings.
pub fn schema() -> Arc<Schema> {
    static SCHEMA: OnceLock<Arc<Schema>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| Arc::new(crate::typed::shape(ID).schema()))
        .clone()
}

/// The GSM receiver: one search component, the stack settings routed to it.
///
/// # Errors
///
/// A [`GraphError`] if a setting of `cfg` is refused.
pub fn build(cfg: &StackConfig) -> Result<StackGraph, GraphError> {
    build_search(
        GsmSearch::default(),
        MIN_RATE_HZ,
        MIN_RATE_MESSAGE,
        cfg,
    )
}

/// Makes the receiver.
pub fn make_receiver(cfg: &StackConfig) -> Result<Box<dyn Receiver>, ConfigError> {
    Ok(Box::new(build(cfg)?))
}

/// Makes the generator of the test signal: the clean signal only. The framework adds noise and offsets.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    let s = &cfg.settings;
    let carriers = gen::scene(u32::from(s.get(&GEN_SCENE).index()));
    Ok(Box::new(gen::GsmGenerator::new(
        cfg.rate_hz,
        s.get(&GEN_CENTRE_HZ),
        carriers,
    )))
}
