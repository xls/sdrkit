//! The interface of the cell family: the shapes of the result sets, the rows and the events, the tables, the event types
//! and what each stack declares.

use sdrkit_core::interface::{
    Arguments, CommandKind, EventKind, Family, Fields, Hertz, Seconds, StackShape, Table, Time,
};
use sdrkit_core::publish::{field, keys};
use sdrkit_core::settings::Argument;
use sdrkit_core::SettingSet;
use sdrkit_graph::DecoderFields;
use sdrkit_util_objects::commands as objcmd;
use sdrkit_util_objects::publish::columns;
use std::sync::{Arc, OnceLock};

// ---- railway_cir ----

sdrkit_core::choice_enum! {
    /// What the railway CIR receiver is doing.
    pub enum RailwayCirState {
        /// No samples yet.
        Waiting = ("waiting", "Waiting for samples"),
        /// Samples, no message in the last 30 s.
        Listening = ("listening", "Listening"),
        /// A message in the last 30 s.
        Decoding = ("decoding", "Decoding"),
    }
}

/// The result fields of `railway_cir`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RailwayCirFields<'a> {
    /// The summary of the last message.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// Messages decoded.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Sync words found.
    #[field(label = "Sync words")]
    pub syncs: i64,
    /// Readings that failed: CRC failures, rejections and ambiguous frames.
    #[field(label = "Failed checks")]
    pub failed_checks: i64,
    /// The carrier offset of the channel.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset_hz: Option<f64>,
    /// The level of the channel.
    #[field(label = "Level", unit = "dB")]
    pub level_db: Option<f64>,
    /// What the receiver is doing.
    #[field(label = "State")]
    pub state: RailwayCirState,
}

/// A row of the message list of `railway_cir`, newest first. The row key is the sequence number of the message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RailwayCirMessageRow<'a> {
    /// The wall time of the reception.
    #[field(label = "Received")]
    pub received: Time,
    /// The protocol: `KLW` or `TDCS`.
    #[field(label = "Protocol")]
    pub protocol: &'a str,
    /// The train (TDCS).
    #[field(label = "Train")]
    pub train: Option<&'a str>,
    /// The locomotive.
    #[field(label = "Locomotive")]
    pub locomotive: &'a str,
    /// The action (KLW).
    #[field(label = "Action")]
    pub action: Option<&'a str>,
    /// The tail device id (KLW).
    #[field(label = "Tail id")]
    pub tail_id: Option<&'a str>,
    /// The brake pipe pressure (KLW).
    #[field(label = "Pressure", unit = "kPa")]
    pub pressure_kpa: Option<f64>,
    /// The speed (TDCS).
    #[field(label = "Speed", unit = "km/h")]
    pub speed_kmh: Option<f64>,
    /// The kilometre post (TDCS).
    #[field(label = "Kilometre post", unit = "km")]
    pub km_post_km: Option<f64>,
    /// The frame marks itself as a test frame (TDCS).
    #[field(label = "Test frame")]
    pub test_frame: bool,
    /// The car count (TDCS).
    #[field(label = "Cars")]
    pub cars: Option<i64>,
    /// The bits corrected.
    #[field(label = "Corrected bits")]
    pub corrections: i64,
    /// The raw frame as hex.
    #[field(label = "Raw frame")]
    pub raw: &'a str,
}

/// A message of the end-of-train device.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RailwayCirKlwEvent<'a> {
    /// The summary: `KLW <action> <locomotive>`.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The locomotive.
    #[field(label = "Locomotive")]
    pub locomotive: &'a str,
    /// The reserved command byte.
    #[field(label = "Reserved byte")]
    pub reserved: i64,
    /// The action code.
    #[field(label = "Action code")]
    pub action_code: i64,
    /// The action.
    #[field(label = "Action")]
    pub action: &'a str,
    /// The parameter as 4 characters.
    #[field(label = "Parameter")]
    pub parameter: Option<&'a str>,
    /// The brake pipe pressure of a pressure action.
    #[field(label = "Pressure", unit = "kPa")]
    pub pressure_kpa: Option<f64>,
    /// The tail device id.
    #[field(label = "Tail id")]
    pub tail_id: Option<&'a str>,
    /// Extra bytes as hex.
    #[field(label = "Extra")]
    pub extra: Option<&'a str>,
    /// The calibration clock, `YYYY-MM-DDThh:mm:ss`.
    #[field(label = "Clock")]
    pub clock: Option<&'a str>,
    /// The bits corrected.
    #[field(label = "Corrected bits")]
    pub corrections: i64,
    /// The raw frame as hex.
    #[field(label = "Raw frame")]
    pub raw: &'a str,
}

/// A dispatch message.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct RailwayCirTdcsEvent<'a> {
    /// The summary: `TDCS <train> <locomotive>`.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The train.
    #[field(label = "Train")]
    pub train: &'a str,
    /// The locomotive.
    #[field(label = "Locomotive")]
    pub locomotive: &'a str,
    /// The locomotive type code.
    #[field(label = "Locomotive type code")]
    pub loco_code: i64,
    /// The locomotive serial.
    #[field(label = "Locomotive serial")]
    pub loco_serial: i64,
    /// The kilometre post.
    #[field(label = "Kilometre post", unit = "km")]
    pub km_post_km: Option<f64>,
    /// The mileage increases.
    #[field(label = "Increasing mileage")]
    pub increasing: bool,
    /// The frame marks itself as a test frame.
    #[field(label = "Test frame")]
    pub test_frame: bool,
    /// The speed.
    #[field(label = "Speed", unit = "km/h")]
    pub speed_kmh: f64,
    /// The weight.
    #[field(label = "Weight", unit = "t")]
    pub weight_t: f64,
    /// The length field divided by 10 (its unit is not published).
    #[field(label = "Length")]
    pub length: f64,
    /// The car count.
    #[field(label = "Cars")]
    pub cars: i64,
    /// The station.
    #[field(label = "Station")]
    pub station: i64,
    /// The track.
    #[field(label = "Track")]
    pub track: i64,
    /// The actual track.
    #[field(label = "Actual track")]
    pub actual_track: i64,
    /// The driver id.
    #[field(label = "Driver")]
    pub driver: i64,
    /// The signal type.
    #[field(label = "Signal type")]
    pub signal_type: i64,
    /// The semaphore type.
    #[field(label = "Semaphore type")]
    pub semaphore_type: i64,
    /// The locomotive role.
    #[field(label = "Locomotive role")]
    pub role: i64,
    /// The bits corrected.
    #[field(label = "Corrected bits")]
    pub corrections: i64,
    /// The raw frame as hex.
    #[field(label = "Raw frame")]
    pub raw: &'a str,
}

/// The messages of `railway_cir`, newest first: a log.
pub static RAILWAY_CIR_MESSAGES: Table<RailwayCirMessageRow<'static>> =
    Table::new("messages", "Messages", crate::railway_cir::HISTORY_MAX)
        .role(sdrkit_core::TableRole::Log);
/// A message of the end-of-train device.
pub static RAILWAY_CIR_KLW: EventKind<RailwayCirKlwEvent<'static>> =
    EventKind::new("railway_cir.klw", "KLW message");
/// A dispatch message.
pub static RAILWAY_CIR_TDCS: EventKind<RailwayCirTdcsEvent<'static>> =
    EventKind::new("railway_cir.tdcs", "TDCS message");

// ---- the search stacks ----

/// The counts and the level of the capture search of every cell stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SearchFields {
    /// Captures searched.
    #[field(label = "Captures searched")]
    pub captures: i64,
    /// Retunes seen (scan steps).
    #[field(label = "Retunes")]
    pub retunes: i64,
    /// Messages decoded.
    #[field(label = "Messages decoded")]
    pub decoded: i64,
    /// Messages that failed their check.
    #[field(label = "Messages failed")]
    pub failed: i64,
    /// The centre frequency of the newest capture.
    #[field(label = "Capture centre", unit = "Hz")]
    pub capture_centre: Option<Hertz>,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
}

/// Where the band search of `cell` and `nr` is, and why nothing shows.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct BandSearchFields<'a> {
    /// The band searched now ("band 28", "n78"). None when no search runs.
    #[field(label = "Band searched")]
    pub search_band: Option<&'a str>,
    /// The radio centre of the search step, counted from 1.
    #[field(label = "Search step")]
    pub search_step: Option<i64>,
    /// The radio centres of the search.
    #[field(label = "Search steps")]
    pub search_steps: Option<i64>,
    /// The passes over every step done since the search started.
    #[field(label = "Search passes")]
    pub search_passes: i64,
    /// Why no cell shows: what the newest capture held, or what the radio gives. None while cells show.
    #[field(label = "Why nothing shows")]
    pub quiet_reason: Option<&'a str>,
}

/// The arguments of `stay` of `cell` and `nr`.
#[derive(Arguments)]
pub struct SearchStayArgs<'a> {
    /// The row key of the cell. Empty: the strongest cell found.
    #[arg(shared = SEARCH_STAY_ROW)]
    pub row: &'a str,
}

/// The row key of the cell to stay on; empty for the strongest.
pub const SEARCH_STAY_ROW: Argument<field::Text> = Argument::new(sdrkit_core::publish::Field::new("row", "Cell", ""))
    .help("The row key of the cell in the cells table; empty for the strongest cell found.");

/// Starts a band search now.
pub const SEARCH_START: sdrkit_core::Command = sdrkit_core::Command::new("search", "Search the bands").help(
    "Steps the radio over the raster of the bands of the setting search_bands (empty: the band of the tuned frequency) and \
     lists every cell found. With band_search on auto the search stays on the strongest cell after one pass.",
);

/// Ends a band search and stays on one cell.
pub const SEARCH_STAY: CommandKind<SearchStayArgs<'static>> = CommandKind::new("stay", "Stay on a cell")
    .help("Ends the band search and tunes to the carrier of a cell of the cells table, or of the strongest cell found.");

/// A cell found or identified by a search stack.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct FoundCellEvent<'a> {
    /// The row key of the cell in the `cells` table.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// What was found.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// The row key of a cell.
pub const SEARCH_CELL_ROW: Argument<field::Text> = objcmd::row_of(crate::decoder::CELLS_KEY);

/// The arguments of `remove` of a search stack.
#[derive(Arguments)]
pub struct SearchRemoveArgs<'a> {
    /// The row key of the cell.
    #[arg(shared = SEARCH_CELL_ROW)]
    pub row: &'a str,
}

/// Forgets one cell.
pub const SEARCH_REMOVE: CommandKind<SearchRemoveArgs<'static>> = objcmd::remove_kind();

/// The fields of every search stack that a change of frequency keeps: the counts of the search.
const SEARCH_KEPT: [&str; 3] = [
    "SearchFields.retunes",
    "SearchFields.decoded",
    "SearchFields.failed",
];

/// The shape of the search stack `id`: the search counts, the result set `F` of the stack and the dropped count.
fn search_stack<F: Fields>(id: &'static str) -> StackShape {
    StackShape::new(id)
        .fields::<SearchFields>()
        .fields::<F>()
        .fields::<DecoderFields>()
}

/// Lists the event, the settings and the commands of a search stack, and what a change of frequency keeps.
fn search_end(
    s: StackShape,
    event: &'static EventKind<FoundCellEvent<'static>>,
    settings: &[&[&'static dyn sdrkit_core::DeclareSetting]],
    kept: &[&'static str],
) -> StackShape {
    let s = settings
        .iter()
        .fold(s.event(event), |s, list| s.settings(list));
    s.command(&sdrkit_graph::decoder::CLEAR)
        .command(&SEARCH_REMOVE)
        .kept(&SEARCH_KEPT)
        .kept(kept)
}

/// [`search_end`] of a stack with a band search: its fields and the commands `search` and `stay`.
fn band_search_end(
    s: StackShape,
    event: &'static EventKind<FoundCellEvent<'static>>,
    settings: &[&[&'static dyn sdrkit_core::DeclareSetting]],
    kept: &[&'static str],
) -> StackShape {
    search_end(
        s.fields::<BandSearchFields>(),
        event,
        settings,
        kept,
    )
    .command(&SEARCH_START)
    .command(&SEARCH_STAY)
}

// ---- tdscdma ----

/// The result fields of `tdscdma`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TdscdmaFields {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
}

/// A row of the cells of `tdscdma`. The row key is the code and the radio centre.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct TdscdmaCellRow<'a> {
    /// The SYNC-DL code.
    #[field(label = "SYNC-DL")]
    pub sync_dl: i64,
    /// The cell parameter, "p / p+1" when only the pair is known.
    #[field(label = "Cell parameter")]
    pub parameter: Option<&'a str>,
    /// The DwPTS level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The DwPTS SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The quadruples counted.
    #[field(label = "P-CCPCH quadruples")]
    pub quadruples: &'a str,
    /// The SFN parity of the first whole frame of the newest capture.
    #[field(label = "Frame parity")]
    pub frame_parity: Option<&'a str>,
    /// The midamble clarity.
    #[field(label = "Midamble clarity")]
    pub clarity: f64,
    /// Captures in which the cell was seen.
    #[field(label = "Captures")]
    pub captures_seen: i64,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    #[field(label = "Frequency")]
    pub frequency: Option<Hertz>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// The cells of `tdscdma`.
pub static TDSCDMA_CELLS: Table<TdscdmaCellRow<'static>> =
    Table::new(crate::decoder::CELLS_KEY, "Cells", crate::tdscdma::CELL_CAP)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&TdscdmaCellRow::NAME);
/// A TD-SCDMA cell found or identified.
pub static TDSCDMA_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.tdscdma_cell", "TD-SCDMA cell");

// ---- umts ----

/// The result fields of `umts`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UmtsFields {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
}

/// A row of the cells of `umts`. The row key is the primary scrambling code and the radio centre.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct UmtsCellRow<'a> {
    /// The primary scrambling code.
    #[field(label = "Code")]
    pub code: i64,
    /// The code group.
    #[field(label = "Group")]
    pub group: i64,
    /// Ec/Io of the CPICH.
    #[field(label = "Ec/Io", unit = "dB")]
    pub ec_io_db: f64,
    /// The CPICH level (RSCP).
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The frame offset.
    #[field(label = "Frame offset", unit = "chips")]
    pub frame_offset: i64,
    /// Whether the cell sends with STTD.
    #[field(label = "STTD")]
    pub sttd: Option<bool>,
    /// Captures in which the cell was seen.
    #[field(label = "Captures")]
    pub captures_seen: i64,
    /// BCH blocks decoded.
    #[field(label = "BCH blocks")]
    pub bch_blocks: i64,
    /// The network.
    #[field(label = "MCC-MNC")]
    pub plmn: Option<&'a str>,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The cell identity.
    #[field(label = "Cell identity")]
    pub cell_id: Option<i64>,
    /// The RNC and the cell of the identity.
    #[field(label = "RNC and cell")]
    pub cell_id_parts: Option<&'a str>,
    /// The UARFCN.
    #[field(label = "UARFCN")]
    pub uarfcn: Option<i64>,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    #[field(label = "Frequency")]
    pub frequency: Option<Hertz>,
    /// The location area code (SIB 1).
    #[field(label = "LAC")]
    pub lac: Option<i64>,
    /// The routing area code (SIB 1).
    #[field(label = "RAC")]
    pub rac: Option<i64>,
    /// The cell is barred (SIB 3).
    #[field(label = "Barred")]
    pub barred: Option<bool>,
    /// The cell is reserved for operator use (SIB 3).
    #[field(label = "Reserved")]
    pub reserved_operator: Option<bool>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// The cells of `umts`.
pub static UMTS_CELLS: Table<UmtsCellRow<'static>> =
    Table::new(crate::decoder::CELLS_KEY, "Cells", crate::umts::CELL_CAP)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&UmtsCellRow::NAME);
/// A UMTS cell found or identified.
pub static UMTS_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.umts_cell", "UMTS cell");

// ---- cdma2000 ----

/// The result fields of `cdma2000`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cdma2000Fields {
    /// Base stations in the table.
    #[field(label = "Base stations", reading = 1)]
    pub station_count: i64,
    /// Base stations with a Sync Channel Message.
    #[field(label = "Identified")]
    pub identified: i64,
}

/// A row of the base stations of `cdma2000`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cdma2000CellRow<'a> {
    /// The PN offset.
    #[field(label = "PN")]
    pub pn: &'a str,
    /// The system identity.
    #[field(label = "SID")]
    pub sid: Option<i64>,
    /// The network identity.
    #[field(label = "NID")]
    pub nid: Option<i64>,
    /// Ec/Io of the pilot.
    #[field(label = "Ec/Io", unit = "dB")]
    pub ec_io_db: f64,
    /// The pilot level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The clock error.
    #[field(label = "Clock error", unit = "ppm")]
    pub clock_ppm: f64,
    /// The paging channel.
    #[field(label = "Paging channel")]
    pub paging: Option<&'a str>,
    /// The protocol revisions.
    #[field(label = "Protocol")]
    pub protocol: Option<&'a str>,
    /// The system time.
    #[field(label = "System time")]
    pub system_time: Option<&'a str>,
    /// The local time.
    #[field(label = "Local time")]
    pub local_time: Option<&'a str>,
    /// The long code state.
    #[field(label = "LC_STATE")]
    pub lc_state: Option<&'a str>,
    /// Messages good of all.
    #[field(label = "Messages")]
    pub messages: &'a str,
    /// When the base station was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The frequency of the carrier: the radio centre and the carrier offset, when the radio centre is known.
    #[field(label = "Frequency")]
    pub frequency: Option<Hertz>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// The base stations of `cdma2000`.
pub static CDMA2000_CELLS: Table<Cdma2000CellRow<'static>> = Table::new(
    crate::decoder::CELLS_KEY,
    "Base stations",
    crate::cdma2000::CELL_CAP,
)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&Cdma2000CellRow::NAME);
/// A base station found or identified.
pub static CDMA2000_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.cdma2000_cell", "CDMA2000 base station");

// ---- nbiot ----

/// The result fields of `nbiot`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NbiotFields {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
    /// Cells with SIB1-NB.
    #[field(label = "Identified")]
    pub identified: i64,
}

/// A row of the cells of `nbiot`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NbiotCellRow<'a> {
    /// The primary network.
    #[field(label = "PLMN")]
    pub plmn: Option<&'a str>,
    /// The operator of the primary network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<i64>,
    /// The EARFCN.
    #[field(label = "EARFCN")]
    pub earfcn: Option<i64>,
    /// The nominal frequency (the offset from 0 Hz when the radio centre is not known).
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The physical cell identity.
    #[field(label = "PCI")]
    pub pci: i64,
    /// The operation mode.
    #[field(label = "Operation mode")]
    pub operation_mode: Option<crate::nbiot::OperationMode>,
    /// The eNB identity.
    #[field(label = "eNB")]
    pub enb: Option<i64>,
    /// The sector.
    #[field(label = "Sector")]
    pub sector: Option<i64>,
    /// The tracking area code.
    #[field(label = "TAC")]
    pub tac: Option<i64>,
    /// Whether the cell is barred.
    #[field(label = "Barred")]
    pub barred: Option<bool>,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// The cells of `nbiot`.
pub static NBIOT_CELLS: Table<NbiotCellRow<'static>> =
    Table::new(crate::decoder::CELLS_KEY, "Cells", crate::nbiot::CELL_CAP)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&NbiotCellRow::NAME);
/// An NB-IoT cell found or identified.
pub static NBIOT_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.nbiot_cell", "NB-IoT cell");

// ---- nr ----

/// The result fields of `nr`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrFields<'a> {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
    /// Cells with a MIB.
    #[field(label = "With MIB")]
    pub with_mib: i64,
    /// Cells with a SIB1.
    #[field(label = "With SIB1")]
    pub with_sib1: i64,
    /// The lowest GSCN the newest capture searched.
    #[field(label = "Lowest GSCN searched")]
    pub gscn_lo: Option<i64>,
    /// The highest GSCN the newest capture searched.
    #[field(label = "Highest GSCN searched")]
    pub gscn_hi: Option<i64>,
    /// The candidates the newest capture searched.
    #[field(label = "Candidates")]
    pub candidates: Option<i64>,
    /// The notice.
    #[field(label = "Notice")]
    pub banner: &'a str,
}

/// A row of the cells of `nr`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrCellRow<'a> {
    /// How far the cell was decoded.
    #[field(label = "Decoded")]
    pub decoded: crate::nr::Decoded,
    /// The first network of SIB1.
    #[field(label = "PLMN")]
    pub plmn: Option<&'a str>,
    /// The operator of that network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The tracking area code of SIB1.
    #[field(label = "TAC")]
    pub tac: Option<i64>,
    /// The NR cell identity of SIB1 (36 bits).
    #[field(label = "Cell identity")]
    pub nci: Option<i64>,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<i64>,
    /// The GSCN.
    #[field(label = "GSCN")]
    pub gscn: Option<i64>,
    /// The frequency of the SSB.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The physical cell identity.
    #[field(label = "PCI")]
    pub pci: i64,
    /// The subcarrier spacing of the SSB.
    #[field(label = "SSB spacing", unit = "Hz")]
    pub ssb_scs: Hertz,
    /// The beams seen.
    #[field(label = "Beams")]
    pub beams: i64,
    /// The SFN of the MIB.
    #[field(label = "SFN")]
    pub sfn: Option<i64>,
    /// Whether the MIB schedules SIB1.
    #[field(label = "SIB1")]
    pub sib1: bool,
    /// The RSRP.
    #[field(label = "RSRP", unit = "dBFS")]
    pub rsrp_dbfs: f64,
    /// The RSRQ.
    #[field(label = "RSRQ", unit = "dB")]
    pub rsrq_db: f64,
    /// The SINR.
    #[field(label = "SINR", unit = "dB")]
    pub sinr_db: f64,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// The details of a cell of `nr`. The row key is the cell key.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrDetailRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// How far the reception of SIB1 went.
    #[field(label = "SIB1")]
    pub sib1: crate::nr::sib1::Sib1Step,
    /// The parts of SIB1 read: 1 the networks, 2 the scheduling, 3 the downlink carrier, 4 the SSB facts.
    #[field(label = "SIB1 parts")]
    pub sib1_parts: Option<i64>,
    /// The width of CORESET#0.
    #[field(label = "CORESET#0 width", unit = "Hz")]
    pub coreset0_width: Option<Hertz>,
    /// The sample rate that holds CORESET#0 with the radio centre of the newest try.
    #[field(label = "Rate for CORESET#0", unit = "Hz")]
    pub rate_needed: Option<Hertz>,
    /// subCarrierSpacingCommon of the MIB.
    #[field(label = "Common spacing", unit = "Hz")]
    pub scs_common: Option<Hertz>,
    /// k_SSB of the MIB.
    #[field(label = "k_SSB")]
    pub k_ssb: Option<i64>,
    /// cellBarred of the MIB.
    #[field(label = "Barred")]
    pub barred: Option<bool>,
    /// intraFreqReselection of the MIB is allowed.
    #[field(label = "Intra-frequency reselection")]
    pub intra_freq_reselection: Option<bool>,
    /// cellReservedForOtherUse of SIB1.
    #[field(label = "Reserved for other use")]
    pub reserved_other_use: Option<bool>,
    /// q-RxLevMin of SIB1.
    #[field(label = "q-RxLevMin", unit = "dBm")]
    pub q_rx_lev_min_dbm: Option<f64>,
    /// The bands of SIB1, apart by spaces.
    #[field(label = "Bands")]
    pub bands: Option<&'a str>,
    /// The SSB period of SIB1.
    #[field(label = "SSB period", unit = "s")]
    pub ssb_period: Option<Seconds>,
    /// The spacing of the carrier of SIB1.
    #[field(label = "Carrier spacing", unit = "Hz")]
    pub carrier_scs: Option<Hertz>,
    /// The width of the carrier of SIB1.
    #[field(label = "Carrier width", unit = "RB")]
    pub carrier_rbs: Option<i64>,
    /// offsetToPointA of SIB1.
    #[field(label = "Offset to point A", unit = "RB")]
    pub offset_to_point_a: Option<i64>,
    /// The first block of the initial downlink BWP.
    #[field(label = "Initial BWP start", unit = "RB")]
    pub bwp_start: Option<i64>,
    /// The width of the initial downlink BWP.
    #[field(label = "Initial BWP width", unit = "RB")]
    pub bwp_rbs: Option<i64>,
    /// The SI window, slots.
    #[field(label = "SI window", unit = "slots")]
    pub si_window: Option<i64>,
    /// Whether SIB1 configures a TDD pattern.
    #[field(label = "TDD")]
    pub tdd: Option<bool>,
    /// SIB1 decodes.
    #[field(label = "SIB1 decodes")]
    pub sib1_count: i64,
    /// The slots of Type0 PDCCH occasions that the newest try searched.
    #[field(label = "SIB1 occasions searched")]
    pub sib1_occasions: Option<i64>,
    /// The control channels with the SI-RNTI that the newest try decoded.
    #[field(label = "SIB1 control channels")]
    pub sib1_pdcch: Option<i64>,
}

/// A network of a cell of `nr`, from SIB1. The row key is the cell key and the place of the network.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrNetworkRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The network.
    #[field(label = "PLMN")]
    pub plmn: &'a str,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The tracking area code.
    #[field(label = "TAC")]
    pub tac: Option<i64>,
    /// The NR cell identity (36 bits).
    #[field(label = "Cell identity")]
    pub nci: i64,
    /// The gNB identity, when SIB1 gives its length.
    #[field(label = "gNB")]
    pub gnb: Option<i64>,
    /// The cell identity within the gNB.
    #[field(label = "Cell in gNB")]
    pub cell_id: Option<i64>,
    /// cellReservedForOperatorUse.
    #[field(label = "Reserved for the operator")]
    pub reserved_operator: bool,
}

/// A SIB that the SIB1 of a cell of `nr` schedules. The row key is the cell key and the SIB number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct NrSibRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The SIB number.
    #[field(label = "SIB")]
    pub sib: i64,
    /// The period of its SI message.
    #[field(label = "Period", unit = "s")]
    pub period: Seconds,
    /// Whether the message is broadcast (else sent on request).
    #[field(label = "Broadcast")]
    pub broadcasting: bool,
    /// The value tag.
    #[field(label = "Value tag")]
    pub value_tag: Option<i64>,
}

/// The cells of `nr`.
pub static NR_CELLS: Table<NrCellRow<'static>> =
    Table::new(crate::decoder::CELLS_KEY, "Cells", crate::nr::CELL_CAP)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&NrCellRow::NAME);
/// The details of the cells of `nr`.
pub static NR_DETAILS: Table<NrDetailRow<'static>> = Table::new("nr_details", "Details", crate::nr::CELL_CAP);
/// The networks of the cells of `nr`.
pub static NR_NETWORKS: Table<NrNetworkRow<'static>> = Table::new("nr_networks", "Networks", 4 * crate::nr::CELL_CAP);
/// The SIBs that the cells of `nr` schedule.
pub static NR_SIBS: Table<NrSibRow<'static>> = Table::new("nr_sibs", "SIBs", 8 * crate::nr::CELL_CAP);
/// An NR cell found or identified.
pub static NR_CELL: EventKind<FoundCellEvent<'static>> = EventKind::new("cell.nr_cell", "NR cell");

// ---- gsmr ----

/// The result fields of `gsmr`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GsmrFields {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
    /// Cells with a location area.
    #[field(label = "Identified")]
    pub identified: i64,
    /// Cells in the railway band.
    #[field(label = "In the railway band")]
    pub railway_cells: i64,
}

/// A row of the cells of `gsmr`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GsmrCellRow<'a> {
    /// The network.
    #[field(label = "PLMN")]
    pub plmn: Option<&'a str>,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The ARFCN.
    #[field(label = "ARFCN")]
    pub arfcn: Option<i64>,
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The base station identity code.
    #[field(label = "BSIC")]
    pub bsic: i64,
    /// The network colour code.
    #[field(label = "NCC")]
    pub ncc: i64,
    /// The base station colour code.
    #[field(label = "BCC")]
    pub bcc: i64,
    /// The location area code.
    #[field(label = "LAC")]
    pub lac: Option<i64>,
    /// The cell identity.
    #[field(label = "Cell")]
    pub cell_id: Option<i64>,
    /// The band of the ARFCN.
    #[field(label = "Band")]
    pub band: Option<crate::gsm::table::GsmBand>,
    /// The level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// A carrier of the newest capture of `gsmr`. The row key is its place, strongest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct GsmrCarrierRow {
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The level over the floor of the spectrum.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
    /// Whether an SCH decoded on the carrier.
    #[field(label = "Synchronised")]
    pub synchronised: bool,
}

/// The cells of `gsmr`.
pub static GSMR_CELLS: Table<GsmrCellRow<'static>> = Table::new(
    crate::decoder::CELLS_KEY,
    "Cells",
    crate::gsm::table::CELL_CAP,
)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&GsmrCellRow::NAME);
/// The carriers of the newest capture of `gsmr`.
pub static GSMR_CARRIERS: Table<GsmrCarrierRow> =
    Table::new("carriers", "Carriers", crate::gsm::table::CARRIER_CAP).sorted(
        &GsmrCarrierRow::LEVEL_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// A GSM cell found or identified.
pub static GSMR_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.gsm_cell", "GSM cell");

// ---- cell (LTE) ----

/// The result fields of `cell`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellFields<'a> {
    /// Cells in the table.
    #[field(label = "Cells", reading = 1)]
    pub cell_count: i64,
    /// Cells with SIB1.
    #[field(label = "Identified")]
    pub identified: i64,
    /// Operators of the cells.
    #[field(label = "Operators")]
    pub operators: i64,
    /// The notice.
    #[field(label = "Notice")]
    pub banner: &'a str,
}

/// A row of the cells of `cell`. The row key is the cell key.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellRow<'a> {
    /// The primary network.
    #[field(label = "PLMN")]
    pub plmn: Option<&'a str>,
    /// The operator of the primary network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The country of the primary network.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The networks of the cell.
    #[field(label = "Networks")]
    pub plmn_count: Option<i64>,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<i64>,
    /// Whether SIB1 confirmed the band.
    #[field(label = "Band confirmed")]
    pub band_confirmed: bool,
    /// The EARFCN.
    #[field(label = "EARFCN")]
    pub earfcn: Option<i64>,
    /// The nominal frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The physical cell identity.
    #[field(label = "PCI")]
    pub pci: i64,
    /// The duplex.
    #[field(label = "Duplex")]
    pub duplex: crate::lte::Duplex,
    /// The eNB identity.
    #[field(label = "eNB")]
    pub enb: Option<i64>,
    /// The sector.
    #[field(label = "Sector")]
    pub sector: Option<i64>,
    /// The cell identity.
    #[field(label = "Cell identity")]
    pub cell_id: Option<i64>,
    /// The tracking area code.
    #[field(label = "TAC")]
    pub tac: Option<i64>,
    /// The bandwidth.
    #[field(label = "Width", unit = "Hz")]
    pub width: Option<Hertz>,
    /// The CRS ports.
    #[field(label = "Ports")]
    pub ports: Option<i64>,
    /// The RSRP.
    #[field(label = "RSRP", unit = "dBFS")]
    pub rsrp_dbfs: f64,
    /// The RSRQ.
    #[field(label = "RSRQ", unit = "dB")]
    pub rsrq_db: f64,
    /// The SINR.
    #[field(label = "SINR", unit = "dB")]
    pub sinr_db: f64,
    /// The carrier offset.
    #[field(label = "Carrier offset", unit = "Hz")]
    pub carrier_offset: Hertz,
    /// Whether the cell is barred.
    #[field(label = "Barred")]
    pub barred: Option<bool>,
    /// Whether SIB1 schedules ETWS.
    #[field(label = "ETWS scheduled")]
    pub etws: bool,
    /// Whether SIB1 schedules CMAS.
    #[field(label = "CMAS scheduled")]
    pub cmas: bool,
    /// When the cell was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// The generation of the radio system.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// Operator, generation, band and physical identity.
    #[field(label = "Name")]
    pub name: &'a str,
}

/// A row of the details of `cell`. The row key is the cell key.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellDetailRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The cyclic prefix.
    #[field(label = "Cyclic prefix")]
    pub cyclic_prefix: crate::lte::Length,
    /// The PHICH duration.
    #[field(label = "PHICH duration")]
    pub phich_duration: Option<crate::lte::Length>,
    /// The PHICH Ng index.
    #[field(label = "PHICH Ng")]
    pub phich_ng: Option<i64>,
    /// The SFN of the first whole frame.
    #[field(label = "SFN")]
    pub sfn: Option<i64>,
    /// The clock error.
    #[field(label = "Clock error", unit = "ppm")]
    pub clock_ppm: f64,
    /// SI messages decoded.
    #[field(label = "SI messages")]
    pub si_messages: i64,
    /// SI messages that failed their CRC.
    #[field(label = "SI failed")]
    pub si_failed: i64,
    /// q-RxLevMin of SIB1.
    #[field(label = "q-RxLevMin", unit = "dBm")]
    pub q_rx_lev_min_dbm: Option<f64>,
    /// p-Max of SIB1.
    #[field(label = "p-Max", unit = "dBm")]
    pub p_max_dbm: Option<f64>,
    /// The SI window.
    #[field(label = "SI window", unit = "s")]
    pub si_window: Option<Seconds>,
    /// The value tag of SIB1.
    #[field(label = "Value tag")]
    pub value_tag: Option<i64>,
    /// The CRS power of SIB2.
    #[field(label = "CRS power", unit = "dBm")]
    pub ref_signal_power_dbm: Option<f64>,
    /// p-b of SIB2.
    #[field(label = "p-b")]
    pub p_b: Option<i64>,
    /// The uplink EARFCN.
    #[field(label = "Uplink EARFCN")]
    pub ul_earfcn: Option<i64>,
    /// The uplink RBs.
    #[field(label = "Uplink RBs")]
    pub ul_rbs: Option<i64>,
    /// The random access preambles.
    #[field(label = "RA preambles")]
    pub ra_preambles: Option<i64>,
    /// The modification period coefficient.
    #[field(label = "Modification period coefficient")]
    pub mod_period_coeff: Option<i64>,
    /// The paging cycle.
    #[field(label = "Paging cycle", unit = "frames")]
    pub paging_cycle: Option<i64>,
    /// t300.
    #[field(label = "t300", unit = "s")]
    pub t300: Option<Seconds>,
    /// t310.
    #[field(label = "t310", unit = "s")]
    pub t310: Option<Seconds>,
    /// n310.
    #[field(label = "n310")]
    pub n310: Option<i64>,
    /// The time alignment timer.
    #[field(label = "Time alignment timer", unit = "s")]
    pub time_alignment: Option<Seconds>,
    /// Whether access class barring is on.
    #[field(label = "Access class barring")]
    pub ac_barring: Option<bool>,
    /// q-Hyst of SIB3.
    #[field(label = "q-Hyst", unit = "dB")]
    pub q_hyst_db: Option<f64>,
    /// The reselection priority of SIB3.
    #[field(label = "Priority")]
    pub priority: Option<i64>,
    /// s-IntraSearch of SIB3.
    #[field(label = "s-IntraSearch", unit = "dB")]
    pub s_intra_search_db: Option<f64>,
    /// s-NonIntraSearch of SIB3.
    #[field(label = "s-NonIntraSearch", unit = "dB")]
    pub s_non_intra_search_db: Option<f64>,
    /// threshServingLow of SIB3.
    #[field(label = "threshServingLow", unit = "dB")]
    pub thresh_serving_low_db: Option<f64>,
    /// t-Reselection of SIB3.
    #[field(label = "t-Reselection", unit = "s")]
    pub t_reselection: Option<Seconds>,
}

/// A network of a cell of `cell`. The row key is the cell key and the place of the network.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellPlmnRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The network.
    #[field(label = "PLMN")]
    pub plmn: &'a str,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
}

/// Another band of a cell of `cell`. The row key is the cell key and the band.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellBandRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The band.
    #[field(label = "Band")]
    pub band: i64,
    /// Whether the band is of the multi-band list.
    #[field(label = "Multi-band list")]
    pub mfbi: bool,
}

/// A system information block of a cell of `cell`. The row key is the cell key and the SIB number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellSibRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The SIB number.
    #[field(label = "SIB")]
    pub sib: i64,
    /// The period of the SIB.
    #[field(label = "Period", unit = "frames")]
    pub period: Option<i64>,
    /// Whether the SIB was read.
    #[field(label = "Read")]
    pub seen: bool,
}

/// A neighbour of a cell of `cell`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellNeighbourRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The list that names the neighbour.
    #[field(label = "List")]
    pub list: crate::lte::NeighbourList,
    /// The EARFCN of the neighbour.
    #[field(label = "EARFCN")]
    pub earfcn: Option<i64>,
    /// The physical cell identity of the neighbour.
    #[field(label = "PCI")]
    pub pci: i64,
    /// q-OffsetCell.
    #[field(label = "q-OffsetCell", unit = "dB")]
    pub q_offset_db: Option<f64>,
    /// The ARFCN of a CDMA2000 neighbour; `pci` is then its PN offset.
    #[field(label = "CDMA2000 ARFCN")]
    pub cdma2000_arfcn: Option<i64>,
}

/// A carrier of another radio system that a cell of `cell` names in SIB6, SIB7, SIB8 or SIB24. The row key is the cell
/// key, the system and the channel number.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellOtherCarrierRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The system.
    #[field(label = "System")]
    pub system: crate::lte::OtherSystem,
    /// The channel number: UARFCN, GSM ARFCN, CDMA2000 ARFCN or NR-ARFCN.
    #[field(label = "Channel")]
    pub channel: i64,
    /// The downlink frequency, where the channel number gives it alone (GSM and NR).
    #[field(label = "Frequency")]
    pub frequency: Option<Hertz>,
    /// The band: dcs1800 or pcs1900 (GSM), bc0 .. (CDMA2000), n78 .. (NR).
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The SSB spacing (NR).
    #[field(label = "SSB spacing")]
    pub ssb_scs: Option<Hertz>,
    /// The reselection priority.
    #[field(label = "Priority")]
    pub priority: Option<i64>,
    /// q-RxLevMin.
    #[field(label = "q-RxLevMin", unit = "dBm")]
    pub q_rx_lev_min_dbm: Option<f64>,
    /// The cells named on the carrier (CDMA2000).
    #[field(label = "Cells")]
    pub cells: i64,
}

/// Another carrier that a cell of `cell` names in SIB5. The row key is the cell key and the EARFCN.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellInterCarrierRow<'a> {
    /// The row key of the cell.
    #[field(label = "Cell", refers_to = "cells")]
    pub cell: &'a str,
    /// The EARFCN.
    #[field(label = "EARFCN")]
    pub earfcn: i64,
    /// The RBs.
    #[field(label = "RBs")]
    pub rbs: i64,
    /// The reselection priority.
    #[field(label = "Priority")]
    pub priority: Option<i64>,
    /// threshX-High.
    #[field(label = "threshX-High", unit = "dB")]
    pub thresh_high_db: f64,
    /// threshX-Low.
    #[field(label = "threshX-Low", unit = "dB")]
    pub thresh_low_db: f64,
    /// q-RxLevMin.
    #[field(label = "q-RxLevMin", unit = "dBm")]
    pub q_rx_lev_min_dbm: f64,
}

/// A site of `cell`: the cells of one eNB. The row key is the network and the eNB.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellSiteRow<'a> {
    /// The network.
    #[field(label = "PLMN")]
    pub plmn: Option<&'a str>,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The eNB identity.
    #[field(label = "eNB")]
    pub enb: i64,
    /// The tracking area code.
    #[field(label = "TAC")]
    pub tac: i64,
    /// The cells of the site.
    #[field(label = "Cells")]
    pub cell_count: i64,
}

/// An operator of `cell`. The row key is the network.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellOperatorRow<'a> {
    /// The network.
    #[field(label = "PLMN")]
    pub plmn: &'a str,
    /// The operator of the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The country of the network.
    #[field(label = "Country")]
    pub country: Option<&'a str>,
    /// The cells of the operator.
    #[field(label = "Cells")]
    pub cell_count: i64,
}

/// A carrier of the newest capture of `cell`. The row key is its place, strongest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct CellCarrierRow {
    /// The frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub frequency: Hertz,
    /// The width.
    #[field(label = "Width", unit = "Hz")]
    pub width: Hertz,
    /// The level.
    #[field(label = "Level", unit = "dB")]
    pub level_db: f64,
}

/// The cells of `cell`.
pub static CELL_CELLS: Table<CellRow<'static>> =
    Table::new(crate::decoder::CELLS_KEY, "Cells", crate::lte::CELL_CAP)
    .role(sdrkit_core::TableRole::Nodes)
    .label(&CellRow::NAME);
/// The details of the cells of `cell`.
pub static CELL_DETAILS: Table<CellDetailRow<'static>> =
    Table::new("details", "Details", crate::lte::CELL_CAP);
/// The networks of the cells of `cell`.
pub static CELL_PLMNS: Table<CellPlmnRow<'static>> =
    Table::new("cell_plmns", "Networks", crate::lte::DETAIL_CAP);
/// The other bands of the cells of `cell`.
pub static CELL_BANDS: Table<CellBandRow<'static>> =
    Table::new("cell_bands", "Other bands", crate::lte::DETAIL_CAP);
/// The system information blocks of the cells of `cell`.
pub static CELL_SIBS: Table<CellSibRow<'static>> = Table::new(
    "cell_sibs",
    "System information blocks",
    crate::lte::DETAIL_CAP,
);
/// The neighbours of the cells of `cell`.
pub static CELL_NEIGHBOURS: Table<CellNeighbourRow<'static>> =
    Table::new("neighbours", "Neighbours", crate::lte::DETAIL_CAP);
/// The other carriers of the cells of `cell`.
pub static CELL_INTER_CARRIERS: Table<CellInterCarrierRow<'static>> =
    Table::new("inter_carriers", "Other carriers", crate::lte::DETAIL_CAP);
/// The carriers of other systems that the cells of `cell` name.
pub static CELL_OTHER_CARRIERS: Table<CellOtherCarrierRow<'static>> =
    Table::new("other_carriers", "Carriers of other systems", crate::lte::DETAIL_CAP);
/// The sites of `cell`.
pub static CELL_SITES: Table<CellSiteRow<'static>> =
    Table::new("sites", "Sites", crate::lte::CELL_CAP);
/// The operators of `cell`.
pub static CELL_OPERATORS: Table<CellOperatorRow<'static>> =
    Table::new("operators", "Operators", crate::lte::CELL_CAP);
/// The carriers of the newest capture of `cell`.
pub static CELL_CARRIERS: Table<CellCarrierRow> =
    Table::new("carriers", "Carriers", crate::lte::CARRIER_CAP).sorted(
        &CellCarrierRow::LEVEL_DB,
        sdrkit_core::SortOrder::Descending,
    );
/// An LTE cell found or identified.
pub static CELL_LTE_CELL: EventKind<FoundCellEvent<'static>> =
    EventKind::new("cell.lte_cell", "LTE cell");

// ---- cv2x ----

/// The result fields of `cv2x`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cv2xFields<'a> {
    /// Whether the sidelink structure is there.
    #[field(label = "Detected")]
    pub detected: bool,
    /// The SC-FDMA symbol length of the channel width.
    #[field(label = "Symbol", unit = "us")]
    pub symbol_us: f64,
    /// The cyclic prefix length of the channel width.
    #[field(label = "Prefix", unit = "us")]
    pub prefix_us: f64,
    /// The slot timing of the detection.
    #[field(label = "Slot timing", unit = "samples")]
    pub slot_timing: Option<i64>,
    /// The prefix correlation of the detection.
    #[field(label = "Prefix correlation")]
    pub correlation: Option<f64>,
    /// The carrier offset of the detection.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// Busy subframes a second.
    #[field(label = "Busy subframes", unit = "1/s")]
    pub busy_per_s: Option<f64>,
    /// The share of busy subframes.
    #[field(label = "Busy share", unit = "%")]
    pub busy_share: Option<f64>,
    /// Whether the last symbol of the subframe is a guard.
    #[field(label = "Guard symbol seen")]
    pub guard_seen: Option<bool>,
    /// The frame shape over the 1 ms subframe.
    #[field(label = "Frame structure")]
    pub frame: Option<&'a str>,
    /// What the stack shows of the content.
    #[field(label = "Content")]
    pub content: &'a str,
    /// The summary.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// What no public document read for the stack confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A detection start of `cv2x`, newest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct Cv2xDetectionRow {
    /// The signal time.
    #[field(label = "Time", unit = "s", role = Time)]
    pub time_s: f64,
    /// The carrier offset.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Hertz,
    /// The prefix correlation.
    #[field(label = "Prefix correlation")]
    pub correlation: f64,
    /// The slot timing.
    #[field(label = "Slot timing", unit = "samples")]
    pub slot_timing: i64,
}

/// The detection starts of `cv2x`: a log.
pub static CV2X_DETECTIONS: Table<Cv2xDetectionRow> =
    Table::new("detections", "Detections", crate::cv2x::DETECTION_CAP)
        .role(sdrkit_core::TableRole::Log);

// ---- eot ----

/// The result fields of `eot`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EotFields<'a> {
    /// Frames shown.
    #[field(shared = keys::FRAMES_OK)]
    pub frames_ok: i64,
    /// Frames the code refused.
    #[field(shared = keys::FRAMES_BAD)]
    pub frames_bad: i64,
    /// Frames with bits repaired.
    #[field(label = "Frames repaired")]
    pub frames_repaired: i64,
    /// Frame syncs.
    #[field(label = "Syncs")]
    pub syncs: i64,
    /// The unit of the newest frame.
    #[field(label = "Last unit")]
    pub last_unit: Option<&'a str>,
    /// The input level.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// What no public document confirms.
    #[field(label = "Note")]
    pub note: &'a str,
}

/// A frame of `eot`, newest first.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EotFrameRow<'a> {
    /// The signal time.
    #[field(label = "Time", unit = "s", role = Time)]
    pub time_s: f64,
    /// The link: end of train or head of train.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The unit.
    #[field(label = "Unit")]
    pub unit: &'a str,
    /// The frame for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
}

/// One frame with every field. The fields of the end-of-train message are absent in a head-of-train frame.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EotFrameEvent<'a> {
    /// The link.
    #[field(label = "Link")]
    pub link: &'a str,
    /// The unit.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The message type.
    #[field(label = "Message type")]
    pub message_type: i64,
    /// The chaining bits.
    #[field(label = "Chaining")]
    pub chaining: Option<i64>,
    /// The battery condition.
    #[field(label = "Battery condition")]
    pub battery: Option<i64>,
    /// The brake pipe pressure.
    #[field(label = "Brake pipe", unit = "psi")]
    pub pressure_psi: Option<i64>,
    /// The battery charge.
    #[field(label = "Battery charge", unit = "%")]
    pub charge_pct: Option<i64>,
    /// The spare bit.
    #[field(label = "Spare")]
    pub spare: Option<bool>,
    /// The valve is open.
    #[field(label = "Valve open")]
    pub valve_open: Option<bool>,
    /// The confirmation bit.
    #[field(label = "Confirmation")]
    pub confirmation: Option<bool>,
    /// The turbine runs.
    #[field(label = "Turbine")]
    pub turbine: Option<bool>,
    /// The train moves.
    #[field(label = "Moving")]
    pub moving: Option<bool>,
    /// The marker battery is good.
    #[field(label = "Marker battery")]
    pub marker_battery: Option<bool>,
    /// The marker light is on.
    #[field(label = "Marker light")]
    pub marker_light: Option<bool>,
    /// The bits repaired.
    #[field(shared = keys::BITS_CORRECTED)]
    pub bits_corrected: i64,
    /// The frame for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
}

/// The frames of `eot`: a log.
pub static EOT_FRAMES: Table<EotFrameRow<'static>> =
    Table::new("frames", "Frames", crate::eot::FRAME_CAP).role(sdrkit_core::TableRole::Log);
/// One frame of `eot`.
pub static EOT_FRAME: EventKind<EotFrameEvent<'static>> =
    EventKind::new("eot.frame", "End-of-train frame");

// ---- lbj ----

/// The result fields of `lbj`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LbjFields {
    /// Bursts with a sync word.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Messages shown.
    #[field(label = "Messages")]
    pub messages: i64,
    /// Messages refused by the checks.
    #[field(label = "Failed checks")]
    pub failed_checks: i64,
    /// Empty frames.
    #[field(label = "Empty frames")]
    pub blank_frames: i64,
    /// The level of the channel.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: Option<f64>,
    /// The SNR of the channel.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
    /// The carrier offset of the newest message.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
}

/// A message of `lbj`. The row key is the place of the message in the order received.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LbjMessageRow<'a> {
    /// When the message came (wall clock), when known.
    #[field(label = "Received")]
    pub received: Option<Time>,
    /// The train (prefix and digits), or the network time of a time message.
    #[field(label = "Train")]
    pub train: Option<&'a str>,
    /// The speed.
    #[field(label = "Speed", unit = "km/h", role = Speed)]
    pub speed_kmh: Option<i64>,
    /// The kilometre post.
    #[field(label = "Km post", unit = "km")]
    pub km_post_km: Option<f64>,
    /// Up or down.
    #[field(label = "Direction")]
    pub direction: Option<crate::lbj::message::Direction>,
    /// The locomotive class and number.
    #[field(label = "Locomotive")]
    pub locomotive: Option<&'a str>,
    /// The route name.
    #[field(label = "Route")]
    pub route: Option<&'a str>,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The level of the burst.
    #[field(shared = keys::LEVEL_DBFS)]
    pub level_dbfs: f64,
}

/// A message of `lbj` shown or completed by its pair.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct LbjMessageEvent<'a> {
    /// The train, or the network time of a time message.
    #[field(shared = keys::SOURCE)]
    pub source: Option<&'a str>,
    /// The message for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The position.
    #[field(shared = keys::POSITION)]
    pub position: Option<(f64, f64)>,
    /// The speed in knots.
    #[field(shared = keys::SPEED_KT)]
    pub speed_kt: Option<f64>,
    /// The speed.
    #[field(label = "Speed", unit = "km/h")]
    pub speed_kmh: Option<i64>,
    /// The kilometre post.
    #[field(label = "Km post", unit = "km")]
    pub km_post_km: Option<f64>,
    /// Up or down.
    #[field(label = "Direction")]
    pub direction: Option<crate::lbj::message::Direction>,
    /// The locomotive class and number.
    #[field(label = "Locomotive")]
    pub locomotive: Option<&'a str>,
    /// The route name.
    #[field(label = "Route")]
    pub route: Option<&'a str>,
}

/// The messages of `lbj`, newest last: a log.
pub static LBJ_MESSAGES: Table<LbjMessageRow<'static>> =
    Table::new("messages", "Messages", crate::lbj::MESSAGE_CAP).role(sdrkit_core::TableRole::Log);
/// A message of `lbj`.
pub static LBJ_MESSAGE: EventKind<LbjMessageEvent<'static>> =
    EventKind::new("lbj.message", "LBJ message");

// ---- ebicab ----

/// The result fields of `ebicab`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EbicabFields {
    /// Bursts.
    #[field(label = "Bursts")]
    pub bursts: i64,
    /// Telegrams decoded.
    #[field(label = "Telegrams")]
    pub telegrams: i64,
    /// Long telegrams.
    #[field(label = "Long telegrams")]
    pub long: i64,
    /// Short telegrams.
    #[field(label = "Short telegrams")]
    pub short: i64,
    /// Telegrams that came inverted.
    #[field(label = "Inverted telegrams")]
    pub inverted: i64,
    /// Bursts without a telegram.
    #[field(label = "Bursts without a telegram")]
    pub bursts_without_telegram: i64,
    /// Parity hits.
    #[field(label = "Parity hits")]
    pub parity_hits: i64,
    /// The bit rate of the newest burst.
    #[field(label = "Bit rate", unit = "bit/s")]
    pub bit_rate_hz: Option<Hertz>,
    /// The carrier offset of the newest burst.
    #[field(shared = keys::OFFSET_HZ)]
    pub offset_hz: Option<Hertz>,
    /// The SNR of the newest burst.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: Option<f64>,
}

/// A telegram of `ebicab`. The row key is the place of the telegram in the order received.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EbicabTelegramRow {
    /// When the telegram came (wall clock), when known.
    #[field(label = "UTC")]
    pub received: Option<Time>,
    /// The length.
    #[field(label = "Length", unit = "bits")]
    pub length_bits: i64,
    /// NID_C.
    #[field(label = "NID_C")]
    pub nid_c: i64,
    /// NID_BG.
    #[field(label = "NID_BG")]
    pub nid_bg: i64,
    /// N_PIG.
    #[field(label = "N_PIG")]
    pub n_pig: i64,
    /// N_TOTAL.
    #[field(label = "N_TOTAL")]
    pub n_total: i64,
    /// M_VERSION.
    #[field(label = "M_VERSION")]
    pub m_version: i64,
    /// The copies received.
    #[field(label = "Copies")]
    pub copies: i64,
    /// The user bits.
    #[field(label = "User bits", content = sdrkit_core::ContentType::Binary)]
    pub user_bits: Arc<[u8]>,
}

/// A balise of `ebicab` heard.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EbicabBaliseRow {
    /// NID_C.
    #[field(label = "NID_C")]
    pub nid_c: i64,
    /// NID_BG.
    #[field(label = "NID_BG")]
    pub nid_bg: i64,
    /// The place of the balise in its group.
    #[field(label = "Balise in group")]
    pub balise_in_group: i64,
    /// The balises of the group.
    #[field(label = "Group size")]
    pub group_size: i64,
    /// When the balise was last heard.
    #[field(shared = columns::LAST_HEARD)]
    pub last_heard: Option<Time>,
    /// Times heard.
    #[field(shared = columns::HEARD)]
    pub heard: Option<i64>,
}

/// A burst of `ebicab` without a telegram. The row key is the place of the burst in the order received.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EbicabOtherBurstRow {
    /// When the burst came (wall clock), when known.
    #[field(label = "UTC")]
    pub received: Option<Time>,
    /// The bits of the burst.
    #[field(label = "Bits")]
    pub bits: i64,
    /// The length of the burst.
    #[field(label = "Duration", unit = "s")]
    pub duration: Seconds,
    /// The SNR.
    #[field(shared = keys::SNR_DB)]
    pub snr_db: f64,
    /// The first bits.
    #[field(label = "First bits", content = sdrkit_core::ContentType::Binary)]
    pub first_bits: Arc<[u8]>,
    /// What the length of the burst suggests.
    #[field(label = "Hint")]
    pub hint: Option<crate::balise::BurstHint>,
}

/// A new telegram of `ebicab`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct EbicabTelegramEvent<'a> {
    /// The balise.
    #[field(shared = keys::SOURCE)]
    pub source: &'a str,
    /// The telegram for people.
    #[field(shared = keys::TEXT)]
    pub text: &'a str,
    /// The family of the source.
    #[field(shared = keys::FAMILY)]
    pub family: keys::Family,
    /// The length.
    #[field(label = "Length", unit = "bits")]
    pub length_bits: i64,
    /// NID_C.
    #[field(label = "NID_C")]
    pub nid_c: i64,
    /// NID_BG.
    #[field(label = "NID_BG")]
    pub nid_bg: i64,
    /// N_PIG.
    #[field(label = "N_PIG")]
    pub n_pig: i64,
}

/// The telegrams of `ebicab`, newest last: a log.
pub static EBICAB_TELEGRAMS: Table<EbicabTelegramRow> =
    Table::new("telegrams", "Telegrams", crate::balise::TELEGRAM_CAP)
        .role(sdrkit_core::TableRole::Log);
/// The balises of `ebicab` heard, newest last.
pub static EBICAB_BALISES: Table<EbicabBaliseRow> =
    Table::new("balises", "Balises", crate::balise::BALISE_CAP);
/// The other bursts of `ebicab`, newest last: a log.
pub static EBICAB_OTHER_BURSTS: Table<EbicabOtherBurstRow> =
    Table::new("other_bursts", "Other bursts", crate::balise::OTHER_CAP)
        .role(sdrkit_core::TableRole::Log);
/// A new telegram of `ebicab`.
pub static EBICAB_TELEGRAM: EventKind<EbicabTelegramEvent<'static>> =
    EventKind::new("balise.telegram", "Balise telegram");

/// The row key of a balise of `ebicab`.
pub const EBICAB_BALISE_ROW: Argument<field::Text> = objcmd::row_of("balises");

/// The arguments of `remove` of `ebicab`.
#[derive(Arguments)]
pub struct EbicabRemoveArgs<'a> {
    /// The row key of the balise.
    #[arg(shared = EBICAB_BALISE_ROW)]
    pub row: &'a str,
}

/// Forgets one balise of `ebicab`.
pub const EBICAB_REMOVE: CommandKind<EbicabRemoveArgs<'static>> = objcmd::remove_kind();

/// A cell of a coverage survey: one row per cell found, in the same columns for every generation. The row key is the key of
/// the cell in the cells table of the stack. The survey table has no role: the cells table is the nodes table of the stack,
/// and a host finds the survey by its key `survey`.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct SurveyRow<'a> {
    /// Operator, generation, band and physical identity.
    #[field(label = "Cell")]
    pub name: &'a str,
    /// The generation.
    #[field(label = "Generation")]
    pub generation: crate::scan::Generation,
    /// The operator name, else the network.
    #[field(label = "Operator")]
    pub operator: Option<&'a str>,
    /// The network.
    #[field(label = "Network")]
    pub plmn: Option<&'a str>,
    /// The band.
    #[field(label = "Band")]
    pub band: Option<&'a str>,
    /// The channel number: ARFCN, UARFCN, EARFCN or NR-ARFCN.
    #[field(label = "Channel")]
    pub channel: Option<i64>,
    /// The frequency.
    #[field(label = "Frequency")]
    pub frequency: Hertz,
    /// The physical identity: PCI, primary scrambling code or BSIC.
    #[field(label = "Physical identity")]
    pub pci: i64,
    /// The cell identity of the broadcast.
    #[field(label = "Cell identity")]
    pub cell_identity: Option<i64>,
    /// The tracking or location area code.
    #[field(label = "Area code")]
    pub area_code: Option<i64>,
    /// RSRP, RSCP or the received level.
    #[field(label = "Level", unit = "dBFS")]
    pub level_dbfs: Option<f64>,
}

/// The survey of `cell`.
pub static CELL_SURVEY: Table<SurveyRow<'static>> = Table::new("survey", "Survey", crate::lte::CELL_CAP)
    .label(&SurveyRow::NAME);
/// The survey of `umts`.
pub static UMTS_SURVEY: Table<SurveyRow<'static>> = Table::new("survey", "Survey", crate::umts::CELL_CAP)
    .label(&SurveyRow::NAME);
/// The survey of `gsmr`.
pub static GSMR_SURVEY: Table<SurveyRow<'static>> = Table::new("survey", "Survey", crate::gsm::table::CELL_CAP)
    .label(&SurveyRow::NAME);
/// The survey of `nr`.
pub static NR_SURVEY: Table<SurveyRow<'static>> = Table::new("survey", "Survey", crate::nr::CELL_CAP)
    .label(&SurveyRow::NAME);

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Family::new("cell", 1, 7)
            .added(
                1,
                1,
                &[
                    "railway_cir.content",
                    "cell.content",
                    "nr.content",
                    "nbiot.content",
                    "gsmr.content",
                    "lbj.content",
                    "ebicab.content",
                    "umts.content",
                    "cdma2000.content",
                    "tdscdma.content",
                    "eot.content",
                    "cv2x.content",
                ],
            )
            // Version 1.4: the SIB1 of NR cells.
            .added(
                1,
                4,
                &[
                    "shape.NrFields.with_sib1",
                    "shape.NrCellRow.decoded",
                    "shape.NrCellRow.plmn",
                    "shape.NrCellRow.operator",
                    "shape.NrCellRow.tac",
                    "shape.NrCellRow.nci",
                    "shape.NrDetailRow",
                    "shape.NrDetailRow.cell",
                    "shape.NrDetailRow.sib1",
                    "shape.NrDetailRow.sib1_parts",
                    "shape.NrDetailRow.coreset0_width",
                    "shape.NrDetailRow.rate_needed",
                    "shape.NrDetailRow.scs_common",
                    "shape.NrDetailRow.k_ssb",
                    "shape.NrDetailRow.barred",
                    "shape.NrDetailRow.intra_freq_reselection",
                    "shape.NrDetailRow.reserved_other_use",
                    "shape.NrDetailRow.q_rx_lev_min_dbm",
                    "shape.NrDetailRow.bands",
                    "shape.NrDetailRow.ssb_period",
                    "shape.NrDetailRow.carrier_scs",
                    "shape.NrDetailRow.carrier_rbs",
                    "shape.NrDetailRow.offset_to_point_a",
                    "shape.NrDetailRow.bwp_start",
                    "shape.NrDetailRow.bwp_rbs",
                    "shape.NrDetailRow.si_window",
                    "shape.NrDetailRow.tdd",
                    "shape.NrDetailRow.sib1_count",
                    "shape.NrNetworkRow",
                    "shape.NrNetworkRow.cell",
                    "shape.NrNetworkRow.plmn",
                    "shape.NrNetworkRow.operator",
                    "shape.NrNetworkRow.tac",
                    "shape.NrNetworkRow.nci",
                    "shape.NrNetworkRow.gnb",
                    "shape.NrNetworkRow.cell_id",
                    "shape.NrNetworkRow.reserved_operator",
                    "shape.NrSibRow",
                    "shape.NrSibRow.cell",
                    "shape.NrSibRow.sib",
                    "shape.NrSibRow.period",
                    "shape.NrSibRow.broadcasting",
                    "shape.NrSibRow.value_tag",
                    "nr.kept.nr_details",
                    "nr.kept.nr_networks",
                    "nr.kept.nr_sibs",
                    "nr.kept.NrFields.with_sib1",
                    "nr.table.nr_details",
                    "nr.table.nr_networks",
                    "nr.table.nr_sibs",
                    "nr.setting.symbol_phase",
                ],
            )
            // Version 1.5: what the newest SIB1 try of an NR cell searched; the carriers of other systems of LTE cells; the survey;
            // the generation, name and frequency of the cells.
            .added(
                1,
                5,
                &[
                    "shape.NrDetailRow.sib1_occasions",
                    "shape.NrDetailRow.sib1_pdcch",
                    "shape.CellNeighbourRow.cdma2000_arfcn",
                    "shape.CellOtherCarrierRow",
                    "shape.CellOtherCarrierRow.cell",
                    "shape.CellOtherCarrierRow.system",
                    "shape.CellOtherCarrierRow.channel",
                    "shape.CellOtherCarrierRow.frequency",
                    "shape.CellOtherCarrierRow.band",
                    "shape.CellOtherCarrierRow.ssb_scs",
                    "shape.CellOtherCarrierRow.priority",
                    "shape.CellOtherCarrierRow.q_rx_lev_min_dbm",
                    "shape.CellOtherCarrierRow.cells",
                    "cell.table.other_carriers",
                    "cell.kept.other_carriers",
                    "shape.SurveyRow",
                    "shape.SurveyRow.name",
                    "shape.SurveyRow.generation",
                    "shape.SurveyRow.operator",
                    "shape.SurveyRow.plmn",
                    "shape.SurveyRow.band",
                    "shape.SurveyRow.channel",
                    "shape.SurveyRow.frequency",
                    "shape.SurveyRow.pci",
                    "shape.SurveyRow.cell_identity",
                    "shape.SurveyRow.area_code",
                    "shape.SurveyRow.level_dbfs",
                    "cell.table.survey",
                    "cell.kept.survey",
                    "nr.table.survey",
                    "nr.kept.survey",
                    "gsmr.table.survey",
                    "gsmr.kept.survey",
                    "umts.table.survey",
                    "umts.kept.survey",
                    "shape.TdscdmaCellRow.frequency",
                    "shape.TdscdmaCellRow.generation",
                    "shape.TdscdmaCellRow.name",
                    "shape.UmtsCellRow.frequency",
                    "shape.UmtsCellRow.lac",
                    "shape.UmtsCellRow.rac",
                    "shape.UmtsCellRow.barred",
                    "shape.UmtsCellRow.reserved_operator",
                    "shape.UmtsCellRow.generation",
                    "shape.UmtsCellRow.name",
                    "shape.Cdma2000CellRow.frequency",
                    "shape.Cdma2000CellRow.generation",
                    "shape.Cdma2000CellRow.name",
                    "shape.NbiotCellRow.generation",
                    "shape.NbiotCellRow.name",
                    "shape.NrCellRow.generation",
                    "shape.NrCellRow.name",
                    "shape.GsmrCellRow.generation",
                    "shape.GsmrCellRow.name",
                    "shape.CellRow.generation",
                    "shape.CellRow.name",
                ],
            )
            .content(
                sdrkit_core::ContentKind::Networks,
                &[
                    "railway_cir",
                    "cell",
                    "nr",
                    "nbiot",
                    "gsmr",
                    "umts",
                    "cdma2000",
                    "tdscdma",
                    "cv2x",
                ],
            )
            .content(sdrkit_core::ContentKind::Tracking, &["lbj"])
            .content(sdrkit_core::ContentKind::Utility, &["ebicab"])
            .content(sdrkit_core::ContentKind::Devices, &["eot"])
            // Version 1.7: the band search of LTE and NR cells, and why nothing shows.
            .added(
                1,
                7,
                &[
                    "shape.BandSearchFields",
                    "shape.BandSearchFields.search_band",
                    "shape.BandSearchFields.search_step",
                    "shape.BandSearchFields.search_steps",
                    "shape.BandSearchFields.search_passes",
                    "shape.BandSearchFields.quiet_reason",
                    "cell.fields.BandSearchFields",
                    "cell.setting.band_search",
                    "cell.setting.search_bands",
                    "cell.setting.search_dwell_s",
                    "cell.command.search",
                    "cell.command.stay",
                    "nr.fields.BandSearchFields",
                    "nr.setting.band_search",
                    "nr.setting.search_bands",
                    "nr.setting.search_dwell_s",
                    "nr.command.search",
                    "nr.command.stay",
                ],
            )
            .shared_source(
                "decoder",
                &["cell", "nr", "nbiot", "gsmr", "umts", "cdma2000", "tdscdma"],
            )
            .shared_source("lte", &["cell"])
            .shared_source("gsm", &["gsmr"])
            .shared_source("balise", &["ebicab"])
            .stack(
                StackShape::new(crate::railway_cir::ID)
                    .fields::<RailwayCirFields>()
                    .fields::<DecoderFields>()
                    .table(&RAILWAY_CIR_MESSAGES)
                    .event(&RAILWAY_CIR_KLW)
                    .event(&RAILWAY_CIR_TDCS)
                    .settings(crate::railway_cir::RailwayCirSetting::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&["messages", "RailwayCirFields.messages"]),
            )
            .stack(band_search_end(
                search_stack::<CellFields>(crate::lte::ID)
                    .table(&CELL_CELLS)
                    .table(&CELL_DETAILS)
                    .table(&CELL_PLMNS)
                    .table(&CELL_BANDS)
                    .table(&CELL_SIBS)
                    .table(&CELL_NEIGHBOURS)
                    .table(&CELL_INTER_CARRIERS)
                    .table(&CELL_SITES)
                    .table(&CELL_OPERATORS)
                    .table(&CELL_CARRIERS)
                    .table(&CELL_OTHER_CARRIERS)
                    .table(&CELL_SURVEY),
                &CELL_LTE_CELL,
                &[crate::lte::LteSetting::SETTINGS, crate::lte::gen::SETTINGS],
                &[
                    "cells",
                    "details",
                    "cell_plmns",
                    "cell_bands",
                    "cell_sibs",
                    "neighbours",
                    "inter_carriers",
                    "other_carriers",
                    "survey",
                    "sites",
                    "operators",
                    "CellFields",
                ],
            ))
            .stack(band_search_end(
                search_stack::<NrFields>(crate::nr::ID)
                    .table(&NR_CELLS)
                    .table(&NR_DETAILS)
                    .table(&NR_NETWORKS)
                    .table(&NR_SIBS)
                    .table(&NR_SURVEY),
                &NR_CELL,
                &[crate::nr::NrSetting::SETTINGS, crate::nr::gen::SETTINGS],
                &[
                    "cells",
                    "nr_details",
                    "nr_networks",
                    "nr_sibs",
                    "survey",
                    "NrFields.cell_count",
                    "NrFields.with_mib",
                    "NrFields.with_sib1",
                    "NrFields.banner",
                ],
            ))
            .stack(search_end(
                search_stack::<NbiotFields>(crate::nbiot::ID).table(&NBIOT_CELLS),
                &NBIOT_CELL,
                &[
                    crate::nbiot::NbSetting::SETTINGS,
                    crate::nbiot::gen::SETTINGS,
                ],
                &["cells", "NbiotFields"],
            ))
            .stack(search_end(
                search_stack::<GsmrFields>(crate::gsm::ID)
                    .table(&GSMR_CELLS)
                    .table(&GSMR_CARRIERS)
                    .table(&GSMR_SURVEY),
                &GSMR_CELL,
                &[crate::gsm::GsmSetting::SETTINGS, crate::gsm::GEN_SETTINGS],
                &["cells", "survey", "GsmrFields"],
            ))
            .stack(
                StackShape::new(crate::lbj::ID)
                    .fields::<LbjFields>()
                    .fields::<DecoderFields>()
                    .decoder("modem")
                    .table(&LBJ_MESSAGES)
                    .event(&LBJ_MESSAGE)
                    .settings(&[&crate::lbj::message::LOCOMOTIVES])
                    .settings(crate::lbj::gen::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&[
                        "messages",
                        "LbjFields.bursts",
                        "LbjFields.messages",
                        "LbjFields.failed_checks",
                        "LbjFields.blank_frames",
                    ]),
            )
            .stack(
                StackShape::new(crate::balise::ID)
                    .fields::<EbicabFields>()
                    .fields::<DecoderFields>()
                    .decoder("uplink")
                    .table(&EBICAB_TELEGRAMS)
                    .table(&EBICAB_BALISES)
                    .table(&EBICAB_OTHER_BURSTS)
                    .event(&EBICAB_TELEGRAM)
                    .settings(crate::balise::gen::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .command(&EBICAB_REMOVE)
                    .kept(&[
                        "telegrams",
                        "balises",
                        "other_bursts",
                        "EbicabFields.bursts",
                        "EbicabFields.telegrams",
                        "EbicabFields.long",
                        "EbicabFields.short",
                        "EbicabFields.inverted",
                        "EbicabFields.bursts_without_telegram",
                        "EbicabFields.parity_hits",
                    ]),
            )
            .stack(search_end(
                search_stack::<UmtsFields>(crate::umts::ID).table(&UMTS_CELLS).table(&UMTS_SURVEY),
                &UMTS_CELL,
                &[crate::umts::UmtsSetting::SETTINGS],
                &["cells", "survey", "UmtsFields"],
            ))
            .stack(search_end(
                search_stack::<Cdma2000Fields>(crate::cdma2000::ID).table(&CDMA2000_CELLS),
                &CDMA2000_CELL,
                &[crate::cdma2000::CdmaSetting::SETTINGS],
                &["cells", "Cdma2000Fields"],
            ))
            .stack(search_end(
                search_stack::<TdscdmaFields>(crate::tdscdma::ID).table(&TDSCDMA_CELLS),
                &TDSCDMA_CELL,
                &[
                    crate::tdscdma::TdSetting::SETTINGS,
                    &[&crate::tdscdma::GEN_MULTIPLIERS],
                ],
                &["cells", "TdscdmaFields"],
            ))
            .stack(
                StackShape::new(crate::eot::ID)
                    .fields::<EotFields>()
                    .fields::<DecoderFields>()
                    .decoder("modem")
                    .table(&EOT_FRAMES)
                    .event(&EOT_FRAME)
                    .settings(&[&crate::eot::DIRECTION])
                    .settings(crate::eot::gen::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR)
                    .kept(&[
                        "frames",
                        "EotFields.frames_ok",
                        "EotFields.frames_bad",
                        "EotFields.frames_repaired",
                        "EotFields.syncs",
                        "EotFields.last_unit",
                    ]),
            )
            .stack(
                StackShape::new(crate::cv2x::ID)
                    .fields::<Cv2xFields>()
                    .fields::<DecoderFields>()
                    .table(&CV2X_DETECTIONS)
                    .settings(&[&crate::cv2x::CHANNEL_MHZ])
                    .settings(crate::cv2x::gen::SETTINGS)
                    .command(&sdrkit_graph::decoder::CLEAR),
            )
    })
}

/// The description of stack `id` of the family.
///
/// # Panics
///
/// If the family does not describe the stack.
pub fn shape(id: &str) -> &'static StackShape {
    family()
        .stack_shape(id)
        .unwrap_or_else(|| panic!("the cell family does not describe the stack {id}"))
}
