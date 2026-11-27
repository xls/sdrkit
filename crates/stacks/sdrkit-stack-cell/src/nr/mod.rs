//! The 5G NR cells stack (`nr`): the SS/PBCH blocks, the MIB and the beams of every cell on the synchronisation raster.
//!
//! Standard: 3GPP TS 38.211, TS 38.212, TS 38.213 and TS 38.331.

pub mod dlsch;
pub mod gen;
pub mod msg;
pub mod phy;
pub mod search;
pub mod sib1;

use crate::bands::{coreset0, hz_to_gscn, nr_band_of};
use crate::decoder::{
    build_search, capture_setting, look_setting, plural, BandList, Capture, FoundCarrier, Headline, Log, SearchConfig, SearchStack,
    FrequencyList, RangeList, Sightings, BAND_ORDER, BAND_SEARCH, SEARCH_BANDS, SEARCH_DWELL, SEARCH_FIRST,
    SEARCH_RANGES,
};
use crate::decoder::RowSync;
use crate::typed::{
    NrCellRow, NrDetailRow, NrFields, NrNetworkRow, NrSibRow, NR_CELL, NR_CELLS, NR_DETAILS, NR_NETWORKS, NR_SIBS,
};
use sdrkit_core::interface::Seconds;
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
use sib1::Sib1Step;
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
    /// How far the reception of SIB1 went at the newest try.
    #[serde(default)]
    pub sib1_step: Sib1Step,
    /// The sample rate that holds CORESET#0 with the radio centre of the newest try, Hz (0: not known).
    #[serde(default)]
    pub sib1_rate_needed_hz: f64,
    /// SIB1, as far as it was read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sib1: Option<Sib1Out>,
    /// SIB1 decodes.
    #[serde(default)]
    pub sib1s: u64,
    /// Slots of Type0 PDCCH occasions searched at the newest try.
    #[serde(default)]
    pub sib1_occasions: u32,
    /// PDCCHs with the SI-RNTI decoded at the newest try.
    #[serde(default)]
    pub sib1_pdcch: u32,
}

/// A network of a cell, from SIB1.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NetworkOut {
    /// MCC and MNC, "262-01".
    pub plmn: String,
    /// The operator of the network, when the list of networks names it.
    pub operator: Option<String>,
    /// The tracking area code.
    pub tac: Option<u32>,
    /// The NR cell identity (36 bits).
    pub nci: u64,
    /// The gNB identity, when SIB1 gives its length.
    pub gnb: Option<u32>,
    /// The cell identity within the gNB, when SIB1 gives the length of the gNB identity.
    pub cell: Option<u16>,
    /// cellReservedForOperatorUse.
    pub reserved_operator: bool,
}

/// A SIB that SIB1 schedules.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SibOut {
    /// The SIB number.
    pub sib: u8,
    /// The period of its SI message, radio frames.
    pub period_frames: u16,
    /// Whether the message is broadcast (else sent on request).
    pub broadcasting: bool,
    /// The value tag.
    pub value_tag: Option<u8>,
}

/// SIB1 as published. `parts` says how much of the message was read: 1 the networks, 2 the scheduling, 3 the downlink
/// carrier, 4 the SSB facts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Sib1Out {
    /// The parts read.
    pub parts: u8,
    /// The networks.
    pub networks: Vec<NetworkOut>,
    /// cellReservedForOtherUse.
    pub reserved_other_use: bool,
    /// q-RxLevMin, dBm.
    pub q_rx_lev_min_dbm: Option<i32>,
    /// The SI window, slots.
    pub si_window_slots: Option<u16>,
    /// The SIBs scheduled.
    pub sibs: Vec<SibOut>,
    /// The bands of the cell.
    pub bands: Vec<u16>,
    /// The spacing of the carrier, kHz.
    pub carrier_scs_khz: Option<u16>,
    /// The width of the carrier, resource blocks.
    pub carrier_rb: Option<u16>,
    /// offsetToPointA, resource blocks.
    pub offset_to_point_a: Option<u16>,
    /// The first block of the initial downlink BWP.
    pub bwp_start_rb: Option<u16>,
    /// The width of the initial downlink BWP, resource blocks.
    pub bwp_rb: Option<u16>,
    /// The SSB period, ms.
    pub ssb_period_ms: Option<u16>,
    /// A TDD pattern is configured.
    pub tdd: Option<bool>,
}

impl Sib1Out {
    /// From the message and the count of its parts read.
    pub fn new(m: &msg::Sib1, parts: u8) -> Sib1Out {
        let mut networks = Vec::new();
        for info in &m.networks {
            let mut prev = None;
            for p in &info.plmns {
                let (mcc, mnc, digits) = p.numbers(prev);
                prev = p.mcc.or(prev);
                let split = info.gnb_and_cell();
                networks.push(NetworkOut {
                    plmn: format!("{mcc:03}-{mnc:0w$}", w = digits as usize),
                    operator: sdrkit_util_ids::mcc::Plmn::new(mcc, mnc, digits)
                        .and_then(sdrkit_util_ids::mcc::operator)
                        .map(str::to_string),
                    tac: info.tac,
                    nci: info.cell_identity,
                    gnb: split.map(|s| s.0),
                    cell: split.map(|s| s.1),
                    reserved_operator: info.reserved_operator,
                });
            }
        }
        let d = m.downlink.as_ref();
        let carrier = d.and_then(|d| d.carriers.first());
        Sib1Out {
            parts,
            networks,
            reserved_other_use: m.reserved_other_use,
            q_rx_lev_min_dbm: m.q_rx_lev_min.map(|v| 2 * i32::from(v)),
            si_window_slots: m.si_window_slots,
            sibs: m
                .si_messages
                .iter()
                .flat_map(|s| {
                    s.sibs.iter().map(|&(sib, value_tag, _)| SibOut {
                        sib,
                        period_frames: s.period_frames,
                        broadcasting: s.broadcasting,
                        value_tag,
                    })
                })
                .collect(),
            bands: d.map(|d| d.bands.iter().copied().filter(|&b| b != 0).collect()).unwrap_or_default(),
            carrier_scs_khz: carrier.map(|c| c.scs_khz),
            carrier_rb: carrier.map(|c| c.bandwidth_rb),
            offset_to_point_a: d.map(|d| d.offset_to_point_a),
            bwp_start_rb: d.map(|d| d.bwp_start_and_width().0),
            bwp_rb: d.map(|d| d.bwp_start_and_width().1),
            ssb_period_ms: m.ssb.map(|s| s.period_ms),
            tdd: m.ssb.map(|s| s.tdd),
        }
    }
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
    /// The sample rate of the newest capture, Hz.
    #[serde(skip)]
    pub rate_hz: f64,
}

impl Default for NrTables {
    fn default() -> Self {
        NrTables {
            cells: ObjectTable::new(CELL_CAP),
            gscn_range: None,
            candidates: None,
            rate_hz: 0.0,
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
    const FORMAT: StateFormat = StateFormat::new("cell.nr_cells", 2);
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
beams, MIB, SIB1). Nothing addressed to a phone is decoded.";

sdrkit_core::choice_enum! {
    /// How far a cell was decoded.
    pub enum Decoded {
        /// The synchronisation signals only.
        Sync = ("sync", "Sync"),
        /// The MIB.
        Mib = ("mib", "MIB"),
        /// The MIB and SIB1.
        Sib1 = ("sib1", "SIB1"),
    }
}

impl NrCell {
    /// How far the cell was decoded.
    pub fn decoded(&self) -> Decoded {
        match (&self.mib, &self.sib1) {
            (_, Some(_)) => Decoded::Sib1,
            (Some(_), None) => Decoded::Mib,
            (None, None) => Decoded::Sync,
        }
    }

    /// The cell as a row of the survey: the first network of SIB1.
    pub fn survey(&self, key: &str) -> crate::scan::SurveyCell {
        let n = self.sib1.as_ref().and_then(|s| s.networks.first());
        crate::scan::SurveyCell {
            key: key.to_string(),
            generation: crate::scan::Generation::Fifth,
            id_word: "PCI",
            plmn: n.map(|n| n.plmn.clone()),
            operator_name: n.and_then(|n| n.operator.clone()),
            band: (self.band > 0).then(|| format!("n{}", self.band)),
            channel: (self.gscn >= 0).then(|| sdrkit_data_plans::cellular::nr_arfcn_of(self.frequency_hz).map(i64::from)).flatten(),
            frequency_hz: self.frequency_hz,
            pci: i64::from(self.pci),
            cell_identity: n.map(|n| n.nci as i64),
            area_code: n.and_then(|n| n.tac).map(i64::from),
            level_dbfs: self.mib.is_some().then_some(self.rsrp_dbfs),
        }
    }

    /// Writes the rows of the cell in the tables of the details, the networks and the SIBs.
    fn write_children(&self, key: &str, t: &mut NrChildren, p: &mut Published) {
        let s = self.sib1.as_ref();
        let m = self.mib.as_ref();
        let bands = s.map(|s| s.bands.iter().map(u16::to_string).collect::<Vec<_>>().join(" "));
        // A try that reached the search of the occasions.
        let searched = matches!(
            self.sib1_step,
            Sib1Step::NoControl | Sib1Step::NoData | Sib1Step::Unreadable | Sib1Step::Decoded
        );
        let width = (self.coreset0_rbs > 0)
            .then(|| m.map(|m| Hertz(f64::from(self.coreset0_rbs * 12 * m.scs_common_khz) * 1e3)))
            .flatten();
        t.details.put(
            p,
            key.to_string(),
            &NrDetailRow {
                cell: key,
                sib1: self.sib1_step,
                sib1_parts: s.map(|s| i64::from(s.parts)),
                coreset0_width: width,
                rate_needed: (self.sib1_rate_needed_hz > 0.0).then_some(Hertz(self.sib1_rate_needed_hz)),
                scs_common: m.map(|m| Hertz(f64::from(m.scs_common_khz) * 1e3)),
                k_ssb: m.map(|m| i64::from(m.k_ssb)),
                barred: m.map(|m| m.barred),
                intra_freq_reselection: m.map(|m| m.intra_freq_reselection),
                reserved_other_use: s.map(|s| s.reserved_other_use),
                q_rx_lev_min_dbm: s.and_then(|s| s.q_rx_lev_min_dbm).map(f64::from),
                bands: bands.as_deref().filter(|b| !b.is_empty()),
                ssb_period: s.and_then(|s| s.ssb_period_ms).map(|ms| Seconds(f64::from(ms) * 1e-3)),
                carrier_scs: s.and_then(|s| s.carrier_scs_khz).map(|k| Hertz(f64::from(k) * 1e3)),
                carrier_rbs: s.and_then(|s| s.carrier_rb).map(i64::from),
                offset_to_point_a: s.and_then(|s| s.offset_to_point_a).map(i64::from),
                bwp_start: s.and_then(|s| s.bwp_start_rb).map(i64::from),
                bwp_rbs: s.and_then(|s| s.bwp_rb).map(i64::from),
                si_window: s.and_then(|s| s.si_window_slots).map(i64::from),
                tdd: s.and_then(|s| s.tdd),
                sib1_count: self.sib1s as i64,
                sib1_occasions: searched.then_some(i64::from(self.sib1_occasions)),
                sib1_pdcch: searched.then_some(i64::from(self.sib1_pdcch)),
            },
        );
        let Some(s) = s else {
            return;
        };
        for (i, n) in s.networks.iter().enumerate() {
            let row = NrNetworkRow {
                cell: key,
                plmn: &n.plmn,
                operator: n.operator.as_deref(),
                tac: n.tac.map(i64::from),
                nci: n.nci as i64,
                gnb: n.gnb.map(i64::from),
                cell_id: n.cell.map(i64::from),
                reserved_operator: n.reserved_operator,
            };
            t.networks.put(p, format!("{key}/{i}"), &row);
        }
        for x in &s.sibs {
            let row = NrSibRow {
                cell: key,
                sib: i64::from(x.sib),
                period: Seconds(f64::from(x.period_frames) * 0.01),
                broadcasting: x.broadcasting,
                value_tag: x.value_tag.map(i64::from),
            };
            t.sibs.put(p, format!("{key}/{}", x.sib), &row);
        }
    }
}

/// The tables that follow the listed cells.
struct NrChildren {
    details: RowSync<NrDetailRow<'static>>,
    networks: RowSync<NrNetworkRow<'static>>,
    sibs: RowSync<NrSibRow<'static>>,
}

impl Shaped for NrCell {
    type Row = NrCellRow<'static>;
    fn row(&self, seen: Option<&Seen>, _now_s: f64, out: RowOut<'_, '_, Self::Row>) {
        let net = self.sib1.as_ref().and_then(|s| s.networks.first());
        out.put(&NrCellRow {
            decoded: self.decoded(),
            plmn: net.map(|n| n.plmn.as_str()),
            operator: net.and_then(|n| n.operator.as_deref()),
            tac: net.and_then(|n| n.tac).map(i64::from),
            nci: net.map(|n| n.nci as i64),
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
            generation: crate::scan::Generation::Fifth,
            name: &self.survey("").label(),
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
    /// The band search settings.
    scan: SearchConfig,
    /// Why the newest capture showed no cell.
    quiet: Option<String>,
    /// Why the cells of the newest capture are read only in part (CORESET#0 wider than the capture).
    partial: Option<String>,
    /// The cells seen by their synchronisation signals only, not yet listed.
    sightings: SyncSightings,
    /// The SSB frequencies of the decoded cells of the newest capture, Hz (absolute when the centre is known).
    decoded_hz: Vec<f64>,
    /// Whether the newest capture left raster points of its centre that no capture on it has searched yet.
    sweep_open: bool,
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
            scan: SearchConfig::new(LOOK_DEFAULT_S),
            quiet: None,
            partial: None,
            sightings: SyncSightings::default(),
            decoded_hz: Vec::new(),
            sweep_open: false,
        }
    }
}

/// A cell seen by its synchronisation signals only and not yet listed.
#[derive(Clone, Copy, Debug)]
struct Sighting {
    key: NrKey,
    /// The signal time of the newest capture that showed it, seconds.
    at_s: f64,
    /// The frequency of its block, Hz.
    hz: f64,
    /// The start of the half frame of its block modulo the SSB period, seconds ([`search::NrFound::frame_s`]).
    frame_s: f64,
    /// The captures in a row that showed it at that frequency and timing.
    count: u32,
}

/// The cells seen by their synchronisation signals only. Such a cell is listed when [`SYNC_ONLY_SIGHTINGS`] captures, each
/// within [`Sightings::WITHIN_S`] of the one before, show its PCI at the same frequency and the same timing modulo the SSB
/// period; a burst of traffic that passes the PSS, the SSS and the DMRS by chance does not come back that way.
#[derive(Clone, Debug, Default)]
struct SyncSightings {
    seen: Vec<Sighting>,
}

/// The captures that show a cell without its MIB before it is listed.
const SYNC_ONLY_SIGHTINGS: u32 = 3;
/// The most frequency difference of two sightings of one cell, Hz.
const SAME_HZ: f64 = 2e3;
/// The most timing difference of two sightings of one cell soon after each other, seconds; it grows with the time between
/// them by the clock error of the radio.
const SAME_TIME_S: f64 = 20e-6;
/// A cell listed by its synchronisation signals alone is no longer listed when searches of its raster point have not shown
/// it for this many seconds of signal.
pub const SYNC_ONLY_KEPT_S: f64 = 5.0;

impl SyncSightings {
    /// The most sightings kept.
    const CAP: usize = 64;

    /// Notes sighting `s` (its `count` is set here). `ppm` is the clock error of the radio as far as known. True when the
    /// cell has now been seen [`SYNC_ONLY_SIGHTINGS`] times in a row.
    fn again(&mut self, mut s: Sighting, ppm: f64) -> bool {
        let t = s.at_s;
        self.seen.retain(|x| t - x.at_s <= Sightings::<NrKey>::WITHIN_S && x.at_s <= t);
        s.count = 1;
        if let Some(i) = self.seen.iter().position(|x| x.key == s.key) {
            let x = self.seen.remove(i);
            let dt = t - x.at_s;
            let d = (s.frame_s - x.frame_s).rem_euclid(search::SSB_PERIOD_S);
            let d = d.min(search::SSB_PERIOD_S - d);
            let tol = SAME_TIME_S + (ppm.abs() + 2.0) * 1e-6 * dt;
            if dt <= 0.0 {
                s.count = x.count;
            } else if (s.hz - x.hz).abs() <= SAME_HZ && d <= tol {
                s.count = x.count + 1;
            }
        }
        if self.seen.len() >= Self::CAP {
            self.seen.remove(0);
        }
        self.seen.push(s);
        s.count >= SYNC_ONLY_SIGHTINGS
    }
}

/// Why a capture of NR `out` at `rate` showed no cell, for people; None when it showed one.
fn quiet_reason(out: &NrSearchOut, rate: f64, threshold_db: f64) -> Option<String> {
    if !out.cells.is_empty() {
        return None;
    }
    if out.sweep_open {
        return Some(format!(
            "searching the {} sync raster positions of this capture in turn, {} in each capture",
            out.candidates,
            out.searched.len()
        ));
    }
    let narrow30 = rate < crate::scan::nr_least_rate(30);
    let note30 = if narrow30 {
        format!(
            "; this capture is too narrow for a 30 kHz SSB (those of n77, n78 and n79, and of some cells of other bands): that needs {:.2} Msps",
            256.0 * 30e3 / 1e6
        )
    } else {
        String::new()
    };
    let best = if out.best_pss_db.is_finite() {
        format!("{:.1} dB", out.best_pss_db)
    } else {
        "none".to_owned()
    };
    if out.best_pss_db >= threshold_db {
        return Some(format!(
            "NR sync seen (PSS {best}) but no cell identity read: the signal is too weak, or the frequency error is too large{note30}"
        ));
    }
    Some(match out.gscn_range {
        Some((a, b)) => format!(
            "no SSB on the {} sync raster positions of this capture (GSCN {a} to {b}; strongest sync {best}, {threshold_db:.0} dB \
             needed){note30}",
            out.candidates
        ),
        None => format!(
            "no sync raster point lies whole in this capture, so only the tuned frequency was tried (strongest sync {best}); a \
             band search steps the raster{note30}"
        ),
    })
}

/// Why the cells of `t` seen at `now` are read only in part: a CORESET#0, which holds the SIB1 and the operator, wider
/// than the capture at `rate`.
fn partial_reason(t: &NrTables, now: f64, rate: f64) -> Option<String> {
    let c = t
        .cells
        .iter()
        .find(|r| r.seen.last_s >= now && r.record.sib1.is_none() && r.record.sib1_step == Sib1Step::TooNarrow)?
        .record;
    let khz = c.mib.as_ref().map_or(15, |m| m.scs_common_khz);
    let width = f64::from(c.coreset0_rbs) * 12.0 * f64::from(khz) * 1e3;
    // With the radio on the cell, as the band search stays: a rate that holds CORESET#0 there is the remedy.
    let remedy = match t.cells.iter().find(|r| r.record.pci == c.pci).and_then(|r| stay_rate_needed(r.key, r.record)) {
        Some(n) if n > rate => format!("raise the rate to {:.0} Msps to read this cell's operator: ", (n / 1e6).ceil()),
        _ => String::new(),
    };
    Some(format!(
        "{remedy}PCI {} sends its SIB1 (the operator) in a CORESET#0 of {} resource blocks ({:.2} MHz): that needs about {:.2} Msps \
         at this centre, at least {:.2} Msps at the centre of CORESET#0; the capture is {:.2} Msps, so only the cell \
         identity, MIB and level are read",
        c.pci,
        c.coreset0_rbs,
        width / 1e6,
        c.sib1_rate_needed_hz / 1e6,
        width / 1e6,
        rate / 1e6
    ))
}

/// The sample rate that holds CORESET#0 of cell `c` with the radio centre on its SS/PBCH block, Hz. None when the MIB schedules
/// no SIB1 or is not known.
fn stay_rate_needed(key: &NrKey, c: &NrCell) -> Option<f64> {
    let m = c.mib.as_ref()?;
    if !c.sib1_scheduled || c.coreset0_rbs == 0 {
        return None;
    }
    let c0 = crate::nr::dlsch::Coreset0 {
        scs_khz: m.scs_common_khz,
        n_rb: c.coreset0_rbs as usize,
        n_symb: c.coreset0_symbols as usize,
        offset_rb: c.coreset0_offset_rb as usize,
    };
    Some(2.0 * (c0.centre_from_ssb(key.scs_khz, m.k_ssb).abs() + c0.width_hz() / 2.0))
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
        if !f.is_block() {
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
        if f.mib.is_none() && !t.cells.contains(&key) {
            // A cell seen by its sync only, beside a decoded SSB that overlaps it, is that SSB seen from a neighbouring
            // raster point; else it is listed when a second capture shows it.
            let width = 240.0 * f64::from(scs) * 1e3;
            let actual = nominal + f.cfo_hz;
            if self.decoded_hz.iter().any(|d| (d - actual).abs() > 20e3 && (d - actual).abs() < width) {
                return;
            }
            let ppm = if known { f.cfo_hz / nominal * 1e6 } else { 0.0 };
            let s = Sighting { key, at_s: time, hz: actual, frame_s: f.frame_s, count: 0 };
            if !self.sightings.again(s, ppm) {
                return;
            }
        }
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
            if c.sib1_step == Sib1Step::NoMib {
                c.sib1_step = if c.sib1_scheduled {
                    Sib1Step::NotTried
                } else {
                    Sib1Step::NotScheduled
                };
            }
        }
        let had_sib1 = c.sib1.is_some();
        // A row made anew (after a clear or a removal) whose SIB1 the searcher tried a short time ago: tried again.
        if new && f.sib1.is_none() {
            self.searcher.forget_sib1(f.pci, key.raster);
        }
        if let Some(r) = &f.sib1 {
            c.sib1_rate_needed_hz = r.rate_needed_hz;
            c.sib1_occasions = r.occasions;
            c.sib1_pdcch = r.pdcch;
            let read = r.payload.as_deref().and_then(msg::Sib1::read);
            match read {
                Some((mut m, parts)) => {
                    // Parts that disagree with the MIB or the block received were read out of place: not kept.
                    let scs = c.mib.as_ref().map_or(0, |m| m.scs_common_khz as u16);
                    let parts = m.plausible_parts(parts, scs, f.anchor_ssb);
                    if parts < 3 {
                        m.downlink = None;
                    }
                    if parts < 4 {
                        m.ssb = None;
                    }
                    c.sib1 = Some(Sib1Out::new(&m, parts));
                    c.sib1_step = Sib1Step::Decoded;
                    c.sib1s += 1;
                }
                // A cell whose SIB1 is known keeps it and its step through a try that fails.
                None if !had_sib1 => {
                    c.sib1_step = match r.step {
                        // A block that passed its CRC and is no SIB1 the description reads.
                        Sib1Step::Decoded => Sib1Step::Unreadable,
                        s => s,
                    }
                }
                None => {}
            }
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
        if let (false, Some(s)) = (had_sib1, &c.sib1) {
            let text = match s.networks.first() {
                Some(n) => format!(
                    "PCI {} at {place}: SIB1, network {}{}, cell identity {}",
                    c.pci,
                    n.plmn,
                    n.tac.map(|t| format!(", tracking area {t}")).unwrap_or_default(),
                    n.nci
                ),
                None => format!("PCI {} at {place}: SIB1", c.pci),
            };
            log.event(time, key.to_string(), text);
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
            NrSetting::SymbolPhase => self.searcher.symbol_phase = SYMBOL_PHASE.read(id, v)?,
            NrSetting::BandOrder => {
                self.order = BandList::read(id, BAND_ORDER.read(id, v)?)?;
                self.searcher.band_order = self.order.bands.clone();
            }
            NrSetting::BandSearch => self.scan.mode = BAND_SEARCH.read(id, v)?,
            NrSetting::SearchBands => self.scan.bands = BandList::read(id, SEARCH_BANDS.read(id, v)?)?,
            NrSetting::SearchFirst => self.scan.first = FrequencyList::read(id, SEARCH_FIRST.read(id, v)?)?,
            NrSetting::SearchRanges => self.scan.ranges = RangeList::read(id, SEARCH_RANGES.read(id, v)?)?,
            NrSetting::SearchDwell => self.scan.dwell_s = SEARCH_DWELL.read(id, v)?,
            NrSetting::SearchLook => self.scan.look_s = SEARCH_LOOK.read(id, v)?,
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
            (NrSetting::SymbolPhase, Value::Bool(self.searcher.symbol_phase)),
            (NrSetting::BandSearch, self.scan.mode_value()),
            (NrSetting::SearchBands, Value::Text(self.scan.bands.text.clone())),
            (NrSetting::SearchFirst, Value::Text(self.scan.first.text.clone())),
            (NrSetting::SearchRanges, Value::Text(self.scan.ranges.text.clone())),
            (NrSetting::SearchDwell, Value::F64(self.scan.dwell_s)),
            (NrSetting::SearchLook, Value::F64(self.scan.look_s)),
        ])
    }
    fn capture_s(&self) -> f64 {
        self.capture_s
    }
    fn search(&mut self, cap: &Capture) -> NrSearchOut {
        self.searcher.search(cap)
    }
    fn bound_search(&mut self, on: bool) {
        self.searcher.bounded = on;
    }
    fn merge(&mut self, t: &mut NrTables, cap: &Capture, found: NrSearchOut, log: &mut Log<Self>) {
        t.gscn_range = found.gscn_range;
        t.candidates = Some(found.candidates);
        t.rate_hz = cap.rate;
        self.quiet = quiet_reason(&found, cap.rate, self.searcher.min_pss_db);
        self.sweep_open = found.sweep_open;
        let base = if cap.centre_known() { cap.centre_hz } else { 0.0 };
        self.decoded_hz = found
            .cells
            .iter()
            .filter(|f| f.mib.is_some())
            .map(|f| base + f.cand.offset_hz + f.cfo_hz)
            .chain(
                t.cells
                    .iter()
                    .filter(|r| r.record.mib.is_some() && cap.end_time - r.seen.last_s <= Sightings::<NrKey>::WITHIN_S)
                    .map(|r| r.record.frequency_hz + r.record.carrier_offset_hz),
            )
            .collect();
        for f in &found.cells {
            self.merge_cell(t, cap, f, log);
        }
        // A cell listed by its sync alone that the searches of its raster point no longer show.
        let now = cap.end_time;
        t.cells.retain(|r| {
            let c = r.record;
            let searched = found.searched.iter().any(|&(hz, scs)| scs == c.ssb_scs_khz && (hz - c.frequency_hz).abs() < 5e3);
            r.favourite || c.mib.is_some() || !searched || now - r.seen.last_s <= SYNC_ONLY_KEPT_S
        });
        self.partial = partial_reason(t, cap.end_time, cap.rate);
    }
    fn found_cells(found: &NrSearchOut) -> usize {
        found.cells.len()
    }
    fn sync_seen(&self, found: &NrSearchOut) -> bool {
        !found.cells.is_empty() || found.best_pss_db >= self.searcher.min_pss_db || found.sweep_open
    }
    fn sweep_open(&self) -> bool {
        self.sweep_open
    }
    fn quiet_reason(&self) -> Option<String> {
        self.quiet.clone()
    }
    fn partial_reason(&self) -> Option<String> {
        self.partial.clone()
    }
    fn band_search(&self) -> Option<&SearchConfig> {
        Some(&self.scan)
    }
    fn search_steps(&self, band: u16, rate: f64, taken: &[f64]) -> Vec<f64> {
        crate::scan::nr_steps(band, rate, &self.scan.ranges.hz, taken)
    }
    fn band_order(&self) -> &[u16] {
        &self.order.bands
    }
    fn bands_at(&self, hz: f64) -> Vec<u16> {
        crate::bands::downlink_bands(sdrkit_data_plans::cellular::CellSystem::Nr, hz, &self.order.bands)
    }
    fn band_name(band: u16) -> String {
        format!("n{band}")
    }
    fn carriers(t: &NrTables) -> Vec<FoundCarrier> {
        t.cells
            .iter()
            .filter(|r| r.record.frequency_hz > 0.0)
            .map(|r| FoundCarrier {
                row: r.key.to_string(),
                frequency_hz: r.record.frequency_hz,
                level_dbfs: r.record.rsrp_dbfs,
                name: format!("PCI {} at {:.2} MHz", r.record.pci, r.record.frequency_hz / 1e6),
                last_s: r.seen.last_s,
                identified: r.record.sib1.is_some(),
                settled: r.record.sib1.is_some()
                    || matches!(
                        r.record.sib1_step,
                        Sib1Step::NotScheduled | Sib1Step::TooNarrow | Sib1Step::NoFrequency
                    ),
                out_of_reach: r.record.sib1_step == Sib1Step::NotScheduled,
                rate_needed_hz: stay_rate_needed(r.key, r.record).filter(|n| r.record.sib1.is_none() && *n > t.rate_hz),
            })
            .collect()
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
        let with_sib1 = t.cells.iter().filter(|r| r.record.sib1.is_some()).count();
        let beams: usize = t.cells.iter().map(|r| r.record.beams.len()).sum();
        p.put(&NrFields {
            cell_count: n as i64,
            with_mib: with_mib as i64,
            with_sib1: with_sib1 as i64,
            gscn_lo: t.gscn_range.map(|r| i64::from(r.0)),
            gscn_hi: t.gscn_range.map(|r| i64::from(r.1)),
            candidates: t.candidates.map(|c| c as i64),
            banner: BANNER_TEXT,
        });
        shaped::rows_where(&t.cells, &NR_CELLS, now, p, |r| self.filter.keep(r.record));
        let mut children = NrChildren {
            details: RowSync::new(&NR_DETAILS),
            networks: RowSync::new(&NR_NETWORKS),
            sibs: RowSync::new(&NR_SIBS),
        };
        let mut survey = Vec::new();
        for r in t.cells.iter().filter(|r| self.filter.keep(r.record)) {
            let key = r.key.to_string();
            survey.push(r.record.survey(&key));
            r.record.write_children(&key, &mut children, p);
        }
        children.details.finish(p);
        children.networks.finish(p);
        children.sibs.finish(p);
        crate::scan::write_survey(&crate::typed::NR_SURVEY, survey, p);
        // The strongest cell by RSRP among those seen within the recent window.
        let best = t
            .cells
            .iter()
            .filter(|r| r.seen.within(now, Self::RECENT_S))
            .max_by(|a, b| a.record.rsrp_dbfs.total_cmp(&b.record.rsrp_dbfs))
            .map(|r| r.record);
        Headline {
            summary: format!(
                "{n} NR cell{}, {with_mib} with MIB, {with_sib1} with SIB1, {beams} beam{}",
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
        /// Whether the symbols carry the phase of up-conversion.
        SymbolPhase = SYMBOL_PHASE,
        /// When the stack searches its bands.
        BandSearch = BAND_SEARCH,
        /// The bands a band search covers.
        SearchBands = SEARCH_BANDS,
        /// The known carriers that a band search visits first.
        SearchFirst = SEARCH_FIRST,
        /// The frequency ranges that a band search keeps to.
        SearchRanges = SEARCH_RANGES,
        /// The longest time on one radio centre of a band search.
        SearchDwell = SEARCH_DWELL,
        /// The first look on each radio centre of a band search.
        SearchLook = SEARCH_LOOK,
    }
}

/// The default capture: four SS burst periods of 20 ms.
const CAPTURE_DEFAULT_S: f64 = 0.08;
/// The default first look of a band search step: four SS burst periods of 20 ms, the period of cells for initial access.
const LOOK_DEFAULT_S: f64 = 0.08;
/// The first look on each radio centre of a band search.
pub const SEARCH_LOOK: Setting<field::F64> = look_setting(LOOK_DEFAULT_S);
/// Seconds of input searched at a time.
pub const CAPTURE_S: Setting<field::F64> = capture_setting(CAPTURE_DEFAULT_S);
/// Cells searched per candidate.
pub const MAX_CELLS: Setting<field::I64> =
    Setting::new("max_cells", "Cells per candidate", "", 6).range(1.0, 12.0, 1.0);
/// The PSS threshold.
pub const MIN_PSS_DB: Setting<field::F64> =
    Setting::new("min_pss_db", "PSS threshold", "dB", 10.0).range(6.0, 20.0, 0.5);
/// Whether the symbols carry the phase of up-conversion to the radio frequency (3GPP TS 38.211 clause 5.4), as a cell sends
/// them. Off reads a signal made without it.
pub const SYMBOL_PHASE: Setting<field::Bool> = Setting::new("symbol_phase", "Symbol phase of up-conversion", "", true)
    .advanced()
    .help("On for cells on the air. Off for a test signal whose symbols all start at phase zero at baseband.");
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
        "5G NR cells needs a sample rate of at least 3.84 Msps (7.68 Msps for 30 kHz SSBs)",
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
