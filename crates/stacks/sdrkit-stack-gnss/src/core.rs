//! The receiver core. It cuts the input into pieces of 16 384 samples and runs, for each piece, input conditioning, every band,
//! the searches, every channel, the measurement epochs that are covered, and the report when it is due. Its products wait in
//! queues for the graph component that wraps it ([`crate::components::GnssCoreComponent`]).

use crate::acq::{AcqHit, AcqPlot, Integration, Search, Searcher};
use crate::codes::{self, E1bTable, SignalKind};
use crate::frontend::{BandBuffer, BandResampler, Conditioner, Excision, SinCos};
use crate::geo::{self, az_el, ecef_to_geodetic};
use crate::msg::NavUnit;
use crate::navstore::{Ephemeris, NavNews, NavStore};
use crate::results::EphemerisState;
use crate::typed::{GnssFixEvent, FIX};
use crate::system::*;
use crate::track::{ChanEvent, Channel, ChannelState};
use sdrkit_core::publish::keys;
use sdrkit_core::{Complex32, ConfigError, Event};
use sdrkit_util_geo::time::LeapSeconds;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

/// Samples per piece.
pub const PIECE: usize = 16_384;
/// The most channels.
pub const MAX_CHANNELS: usize = 40;
/// The most rounds one search start ends while it looks for a satellite that is not tracked. A GPS round with a fix holds only
/// the predicted satellites, and every sixth round holds all of them.
const MAX_EMPTY_ROUNDS: u32 = 6;
/// A locked L1 C/A channel this many dB below another one at the same Doppler (modulo 1 kHz) is a cross-correlation.
const XCORR_DB: f64 = 20.0;
/// The Doppler match of a cross-correlation, Hz.
const XCORR_HZ: f64 = 20.0;
/// How long a satellite dropped as a cross-correlation is not searched, s.
const XCORR_HOLD_S: f64 = 30.0;
/// Knots in one metre a second.
const MS_TO_KT: f64 = 3600.0 / 1852.0;
/// The report period, s of signal.
pub const REPORT_S: f64 = 0.25;

/// The receiver's configuration (from the settings).
#[derive(Clone, Debug)]
pub struct CoreConfig {
    pub systems: u8,
    pub elevation_mask: f64,
    pub search_half_width: f64,
    pub freq_hint: Option<f64>,
    pub effort: f64,
    pub approx_position: Option<(f64, f64)>,
    pub week_ref: i64,
    pub clock_compare: bool,
    pub galileo: Option<E1bTable>,
    /// The `sbas` stack: the systems are SBAS alone and the plan is the widening one.
    pub sbas_mode: bool,
    /// The station's leap-second table: GPS - UTC until the satellites broadcast it.
    pub leap_table: LeapSeconds,
}

impl Default for CoreConfig {
    fn default() -> Self {
        CoreConfig {
            systems: System::ALL.iter().fold(0, |m, s| m | s.bit()),
            elevation_mask: 5.0,
            search_half_width: 10_000.0,
            freq_hint: None,
            effort: 10.0,
            approx_position: None,
            week_ref: 2400,
            clock_compare: false,
            galileo: None,
            sbas_mode: false,
            leap_table: LeapSeconds::BUILTIN,
        }
    }
}

/// A navigation frame on the `gnss.nav_frame` stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavFrame {
    pub sat: SatId,
    pub unit: NavUnit,
    /// Satellite time (of week; of day for GLONASS) of the first bit, when the unit carries one.
    pub time: Option<f64>,
    pub start_epoch: u64,
}

/// One satellite of a measurement epoch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SatMeas {
    pub sat: SatId,
    pub glo_k: Option<i8>,
    /// Satellite transmit time (its own clock; seconds of its system's week, of the day for GLONASS).
    pub tx_time: f64,
    pub doppler: f64,
    pub carrier: f64,
    pub cn0: f64,
    pub used: bool,
    pub residual: Option<f64>,
}

/// The `gnss.measurement` stream: once a second.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeasEpoch {
    /// Receive instant, seconds of signal.
    pub rx_time: f64,
    pub sats: Vec<SatMeas>,
    /// GPS week and time of week of the receive instant, when a fix has set the clock.
    pub gps_time: Option<(i64, f64)>,
}

/// Per system in a fix.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SystemFix {
    pub system: Option<System>,
    pub in_fix: bool,
    pub satellites: usize,
    pub clock_m: f64,
}

/// The `gnss.fix` stream.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fix {
    pub valid: bool,
    pub lat: f64,
    pub lon: f64,
    pub height: f64,
    pub ecef: [f64; 3],
    pub systems: Vec<SystemFix>,
    pub galileo_offset_ns: Option<f64>,
    pub bdt_offset_ns: Option<f64>,
    pub glo_tau_gps_ns: Option<f64>,
    pub glo_tau_c_ns: Option<f64>,
    pub hdop: f64,
    pub vdop: f64,
    pub pdop: f64,
    pub tdop: f64,
    pub herr: f64,
    pub verr: f64,
    pub rms: f64,
    pub type_text: String,
    pub first_fix_s: f64,
    pub fix_count: u64,
    pub week: i64,
    pub tow: f64,
    /// GPS - UTC, s: broadcast when `leap_known`, else from the station's leap-second table.
    pub leap: Option<i32>,
    /// Whether the satellites broadcast `leap`.
    pub leap_known: bool,
    /// UTC of the fix, seconds since 1970; `None` while GPS - UTC is not known.
    pub utc: Option<f64>,
    pub has_velocity: bool,
    pub speed: f64,
    pub course: Option<f64>,
    pub vel_enu: [f64; 3],
    pub clock_bias_m: f64,
    pub clock_drift_ms: f64,
    pub signal_s: f64,
    pub satellites: usize,
    pub excluded: usize,
}

/// An SBAS message on the `sbas.message` stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SbasMessage {
    pub prn: u16,
    pub bits: Vec<u8>,
    pub msg_type: u8,
    pub signal_s: f64,
    pub cn0: f64,
    pub doppler: f64,
}

/// A channel row (also the `gnss.channels` stream).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChannelRow {
    pub sat: Option<SatId>,
    pub name: String,
    pub system: Option<System>,
    pub glo_k: Option<i8>,
    pub cn0: f64,
    pub doppler: f64,
    pub code_chips: f64,
    pub az: Option<f64>,
    pub el: Option<f64>,
    pub state: Option<ChannelState>,
    pub state_level: u8,
    pub lock_s: f64,
    pub good: u64,
    pub bad: u64,
    pub residual: Option<f64>,
    /// The satellite says it is healthy (`None` while it is not known).
    pub health: Option<bool>,
    pub in_fix: bool,
    pub code_freq: f64,
    pub cn0_history: Vec<f32>,
}

/// A navigation row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavRow {
    pub name: String,
    pub ephemeris: Option<EphemerisState>,
    /// Ephemeris parts collected and needed.
    pub parts: Option<(u32, u32)>,
    /// About how long until the ephemeris is complete, s.
    pub seconds_left: Option<f64>,
    pub iod: Option<i64>,
    pub age_s: Option<f64>,
    pub week: Option<i64>,
    pub tow: Option<f64>,
    pub af0_us: Option<f64>,
}

/// A sky entry.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkyEntry {
    pub system: Option<System>,
    pub prn: u16,
    pub name: String,
    pub glo_k: Option<i8>,
    pub az: f64,
    pub el: f64,
    pub tracked: bool,
    pub used: bool,
    pub cn0: f64,
    pub healthy: bool,
    pub from_ephemeris: bool,
}

/// A satellite sub-point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SubPoint {
    pub name: String,
    pub system: Option<System>,
    pub lat: f64,
    pub lon: f64,
    pub height_km: f64,
    pub tracked: bool,
    pub used: bool,
    pub cn0: f64,
    pub healthy: bool,
    pub from_ephemeris: bool,
    pub age_s: f64,
    /// Offered to a shared map (a valid broadcast ephemeris, a known slot for GLONASS).
    pub public: bool,
}

/// The search state of the report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchInfo {
    pub running: bool,
    pub system: Option<System>,
    pub prn: u16,
    pub progress: f64,
    pub rounds: u32,
    pub centre_hz: f64,
    pub half_hz: f64,
    pub ms: f64,
    pub stage: u8,
    pub pulling_in: usize,
    pub first_lock_s: Option<f64>,
    pub work_units: f64,
}

/// Front end numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrontEndInfo {
    pub level_dbfs: f64,
    pub clip_percent: f64,
    pub dc_i: f64,
    pub dc_q: f64,
    pub bins_cut: u64,
}

/// The receiver's report: everything its screens show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GnssReport {
    pub input_rate: f64,
    pub centre_hz: f64,
    pub wanted_mask: u8,
    pub active_mask: u8,
    pub searched_mask: u8,
    pub signal_s: f64,
    pub state: u8,
    pub channels: Vec<ChannelRow>,
    pub sky: Vec<SkyEntry>,
    pub satellites: Vec<SubPoint>,
    pub navigation: Vec<NavRow>,
    pub fix: Option<Fix>,
    pub iono_valid: bool,
    pub iono: Option<geo::Klobuchar>,
    pub utc_valid: bool,
    pub utc: Option<crate::lnav::UtcParams>,
    pub leap_announcement: Option<(i32, u32, u32)>,
    pub clock_offset_ms: Option<f64>,
    pub clock_offset_avg_ms: Option<f64>,
    pub level_history: Vec<f32>,
    pub cn0_history: Vec<f32>,
    pub clock_history: Vec<f32>,
    pub almanac_count: usize,
    pub sbas_total: u64,
    pub search: SearchInfo,
    pub acq_plot: Option<AcqPlot>,
    pub scatter: Vec<[f32; 2]>,
    pub scatter_sat: String,
    pub front_end: FrontEndInfo,
    pub status_line: String,
    pub progress: Vec<String>,
    pub frequency_error_hz: Option<f64>,
    pub no_band: bool,
}

/// Which band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BandId {
    L1,
    B1i,
    Glo(i8),
}

struct Band {
    id: BandId,
    rate: f64,
    stage1: Option<usize>,
    resampler: BandResampler,
    excision: Option<Excision>,
    buf: BandBuffer,
    tmp: Vec<Complex32>,
}

/// A GLONASS wide cut (above 16 Msps).
struct Wide {
    resampler: BandResampler,
    rate: f64,
    centre_off: f64,
    out: Vec<Complex32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Gps,
    Aux,
    Galileo,
    Beidou,
    Glo(i8),
}

impl Family {
    fn system(self, prn: u16) -> System {
        match self {
            Family::Gps => System::Gps,
            Family::Aux => System::of_l1_prn(prn).unwrap_or(System::Sbas),
            Family::Galileo => System::Galileo,
            Family::Beidou => System::Beidou,
            Family::Glo(_) => System::Glonass,
        }
    }
}

struct Slot {
    family: Family,
    band: usize,
    kind: SignalKind,
    carrier: f64,
    searcher: Searcher,
    prns: Vec<u16>,
    order: Vec<u16>,
    pos: usize,
    round: u32,
    found_round: bool,
    prev_found: bool,
    job: Option<Search>,
    spare: Option<Search>,
    confirming: Option<AcqHit>,
    budget: f64,
    cap: f64,
    last_seg_end: u64,
    stage: u8,
    sleep_until: f64,
    sleep_len: f64,
    long_round: bool,
    current: Option<u16>,
    window: (f64, f64, usize),
}

/// The receiver core.
pub struct GnssCore {
    pub cfg: CoreConfig,
    fs: f64,
    centre: f64,
    live: bool,
    ready: bool,
    lut: SinCos,
    pending: Vec<Complex32>,
    piece: Vec<Complex32>,
    pieces: u64,
    cond: Conditioner,
    bands: Vec<Band>,
    wide: Option<Wide>,
    slots: Vec<Slot>,
    pub channels: Vec<Channel>,
    pub nav: NavStore,
    active_mask: u8,
    searched_mask: u8,
    no_band: bool,
    next_meas: f64,
    next_report: f64,
    pub fix: Option<Fix>,
    last_fix_s: f64,
    first_fix_s: Option<f64>,
    fix_count: u64,
    clock_offset: Option<f64>,
    pub freq_error: Option<f64>,
    worked_centre: Option<f64>,
    first_lock_s: Option<f64>,
    good_total: u64,
    bad_total: u64,
    sbas_total: u64,
    glo_rot: usize,
    pub log: VecDeque<String>,
    pub events: Vec<Event>,
    plot: Option<AcqPlot>,
    // Products for the graph.
    pub out_hits: Vec<(f64, AcqHit)>,
    pub out_frames: Vec<(f64, NavFrame)>,
    pub out_eph: Vec<(f64, (SatId, Ephemeris))>,
    pub out_meas: Vec<(f64, MeasEpoch)>,
    pub out_fix: Vec<(f64, Fix)>,
    pub out_sbas: Vec<(f64, SbasMessage)>,
    pub out_channels: Vec<(f64, Vec<ChannelRow>)>,
    pub report: GnssReport,
    pub report_due: bool,
    level_hist: VecDeque<f32>,
    cn0_hist: VecDeque<f32>,
    clock_hist: VecDeque<f32>,
    clock_offsets: VecDeque<f64>,
    clock_min: (f64, f64, f64),
    last_used: BTreeMap<SatId, (f64, f64, f64)>,
    sky_pred: Vec<SatId>,
    prior: Option<[f64; 3]>,
    chev: Vec<ChanEvent>,
    /// Satellites dropped as cross-correlations, with the signal time until which they are not searched.
    xcorr: Vec<(SatId, f64)>,
    reset_marker: f64,
    solver: crate::fixsolve::FixSolver,
}

/// The lowest and the highest of `v`, or `None` when it is empty.
fn span(v: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    v.fold(None, |m, x| match m {
        None => Some((x, x)),
        Some((lo, hi)) => Some((f64::min(lo, x), f64::max(hi, x))),
    })
}

impl GnssCore {
    /// GPS - UTC at GPS time of week `tow`: broadcast, else from the station's leap-second table. `None` when neither provides
    /// it.
    pub fn leap_s(&self, tow: f64) -> Option<i32> {
        let week = self.nav.gps_week().unwrap_or(self.cfg.week_ref);
        self.nav
            .utc
            .map(|u| u.dtls)
            .or_else(|| geo::leap_at(week, tow, &self.cfg.leap_table).ok())
    }

    /// Takes the station's leap-second table.
    pub fn set_leap_table(&mut self, table: LeapSeconds) {
        self.cfg.leap_table = table;
        self.solver.leap_table = table;
    }

    pub fn new(cfg: CoreConfig) -> GnssCore {
        let week_ref = cfg.week_ref;
        let mut c = GnssCore {
            nav: NavStore::new(cfg.week_ref),
            cfg,
            fs: 0.0,
            centre: L1_HZ,
            live: false,
            ready: false,
            lut: SinCos::new(),
            pending: Vec::with_capacity(PIECE),
            piece: vec![Complex32::new(0.0, 0.0); PIECE],
            pieces: 0,
            cond: Conditioner::default(),
            bands: Vec::new(),
            wide: None,
            slots: Vec::new(),
            channels: Vec::new(),
            active_mask: 0,
            searched_mask: 0,
            no_band: false,
            next_meas: 1.0,
            next_report: REPORT_S,
            fix: None,
            last_fix_s: 0.0,
            first_fix_s: None,
            fix_count: 0,
            clock_offset: None,
            freq_error: None,
            worked_centre: None,
            first_lock_s: None,
            good_total: 0,
            bad_total: 0,
            sbas_total: 0,
            glo_rot: 0,
            log: VecDeque::new(),
            events: Vec::new(),
            plot: None,
            out_hits: Vec::new(),
            out_frames: Vec::new(),
            out_eph: Vec::new(),
            out_meas: Vec::new(),
            out_fix: Vec::new(),
            out_sbas: Vec::new(),
            out_channels: Vec::new(),
            report: GnssReport::default(),
            report_due: false,
            level_hist: VecDeque::new(),
            cn0_hist: VecDeque::new(),
            clock_hist: VecDeque::new(),
            clock_offsets: VecDeque::new(),
            clock_min: (f64::INFINITY, f64::INFINITY, 0.0),
            last_used: BTreeMap::new(),
            sky_pred: Vec::new(),
            prior: None,
            chev: Vec::new(),
            xcorr: Vec::with_capacity(MAX_CHANNELS),
            reset_marker: 0.0,
            solver: crate::fixsolve::FixSolver::new(2400, 5.0),
        };
        c.solver = crate::fixsolve::FixSolver::new(week_ref, c.cfg.elevation_mask);
        c.solver.leap_table = c.cfg.leap_table;
        c
    }

    fn logl(&mut self, s: impl Into<String>) {
        let s = s.into();
        self.log.push_back(s);
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }

    /// Signal time, s.
    pub fn signal_s(&self) -> f64 {
        if self.fs > 0.0 {
            self.pieces as f64 * PIECE as f64 / self.fs
        } else {
            0.0
        }
    }

    /// The minimum input rate.
    pub const MIN_RATE: f64 = 2.046e6;

    /// Configures for an input rate and centre. Refuses a rate under 2.046 Msps.
    ///
    /// # Errors
    ///
    /// [`ConfigError::RateTooLow`] below [`GnssCore::MIN_RATE`].
    pub fn configure(&mut self, fs: f64, centre: f64, live: bool) -> Result<(), ConfigError> {
        self.live = live;
        if fs < Self::MIN_RATE {
            self.ready = false;
            self.bands.clear();
            self.slots.clear();
            self.channels.clear();
            self.active_mask = 0;
            return Err(ConfigError::RateTooLow {
                need: Self::MIN_RATE,
                got: fs,
            });
        }
        let changed = (fs - self.fs).abs() > 1e-6 || (centre - self.centre).abs() > 1e-3;
        self.fs = fs;
        self.centre = centre;
        self.ready = true;
        if changed || self.bands.is_empty() {
            self.plan();
        }
        Ok(())
    }

    /// Whether the receiver accepted its rate.
    pub fn ready(&self) -> bool {
        self.ready
    }

    fn wanted(&self, s: System) -> bool {
        self.cfg.systems & s.bit() != 0
    }

    /// Plans the bands for the centre, rate and systems.
    pub fn plan(&mut self) {
        let fs = self.fs;
        let mut bands = Vec::new();
        let mut active = 0u8;
        let l1_wanted =
            self.wanted(System::Gps) || self.wanted(System::Qzss) || self.wanted(System::Sbas);
        let off_l1 = L1_HZ - self.centre;
        let l1_ok = fs >= 2.0e6 && off_l1.abs() + 1.0e6 <= fs / 2.0;
        let gal_ok = l1_ok && self.wanted(System::Galileo) && fs >= 3.5e6;
        if l1_ok && (l1_wanted || gal_ok) {
            bands.push(BandId::L1);
            active |= System::Gps.bit();
            if gal_ok {
                active |= System::Galileo.bit();
            }
        }
        let off_b = B1I_HZ - self.centre;
        if self.wanted(System::Beidou) && fs >= 3.9e6 && off_b.abs() + 1.0e6 <= fs / 2.0 {
            bands.push(BandId::B1i);
            active |= System::Beidou.bit();
        }
        if self.wanted(System::Glonass) && fs >= 1.9e6 {
            for k in -7..=6i8 {
                let o = glo_carrier(k) - self.centre;
                if o.abs() + 0.55e6 <= fs / 2.0 {
                    bands.push(BandId::Glo(k));
                    active |= System::Glonass.bit();
                }
            }
        }
        // Keep the bands (and their channels) that stay; drop the others.
        let old: Vec<BandId> = self.bands.iter().map(|b| b.id).collect();
        if old != bands {
            let keep: Vec<bool> = self.bands.iter().map(|b| bands.contains(&b.id)).collect();
            let mut ev = Vec::new();
            let band_ids: Vec<BandId> = self.bands.iter().map(|b| b.id).collect();
            let chans = std::mem::take(&mut self.channels);
            for mut c in chans {
                let bid = self.band_of(&c, &band_ids);
                if bid.is_some_and(|b| keep[b]) && !self.bands.is_empty() {
                    self.channels.push(c);
                } else {
                    c.drop_with(&mut ev, "band dropped");
                }
            }
            self.make_bands(&bands);
        }
        self.active_mask = active;
        self.no_band = bands.is_empty();
        self.make_slots();
    }

    fn band_of(&self, c: &Channel, ids: &[BandId]) -> Option<usize> {
        let want = match c.sat.system {
            System::Beidou => BandId::B1i,
            System::Glonass => BandId::Glo(c.glo_k()),
            _ => BandId::L1,
        };
        ids.iter().position(|b| *b == want)
    }

    fn make_bands(&mut self, ids: &[BandId]) {
        let fs = self.fs;
        self.bands.clear();
        self.wide = None;
        let glo: Vec<i8> = ids
            .iter()
            .filter_map(|b| {
                if let BandId::Glo(k) = b {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();
        if let (true, Some(&k_lo), Some(&k_hi)) = (fs > 16e6, glo.iter().min(), glo.iter().max()) {
            let lo = glo_carrier(k_lo) - self.centre;
            let hi = glo_carrier(k_hi) - self.centre;
            let span = hi - lo;
            let rate = 1.024e6 * ((span + 1.4e6) / 1.024e6).ceil();
            let mid = 0.5 * (lo + hi);
            self.wide = Some(Wide {
                resampler: BandResampler::new(fs, rate, mid),
                rate,
                centre_off: mid,
                out: Vec::new(),
            });
        }
        for &id in ids {
            let (rate, offset) = match id {
                BandId::L1 => (4.096e6, L1_HZ - self.centre),
                BandId::B1i => (4.096e6, B1I_HZ - self.centre),
                BandId::Glo(k) => (1.024e6, glo_carrier(k) - self.centre),
            };
            let (resampler, stage1) = match (&self.wide, id) {
                (Some(w), BandId::Glo(_)) => (
                    BandResampler::new(w.rate, rate, offset - w.centre_off),
                    Some(0),
                ),
                _ => (BandResampler::new(fs, rate, offset), None),
            };
            self.bands.push(Band {
                id,
                rate,
                stage1,
                resampler,
                excision: matches!(id, BandId::L1 | BandId::B1i).then(Excision::default),
                buf: BandBuffer::default(),
                tmp: Vec::with_capacity(PIECE * 2),
            });
        }
    }

    fn make_slots(&mut self) {
        let mut slots: Vec<Slot> = Vec::new();
        let mut keep_old = std::mem::take(&mut self.slots);
        let mut take = |fam: Family| -> Option<Slot> {
            keep_old
                .iter()
                .position(|s| s.family == fam)
                .map(|i| keep_old.remove(i))
        };
        let mut searched = 0u8;
        for (bi, b) in self.bands.iter().enumerate() {
            match b.id {
                BandId::L1 => {
                    let gps = self.wanted(System::Gps) && !self.cfg.sbas_mode;
                    let mut aux_prns: Vec<u16> = Vec::new();
                    if self.wanted(System::Qzss) {
                        aux_prns.extend(193..=202);
                    }
                    if self.wanted(System::Sbas) {
                        aux_prns.extend(120..=158);
                    }
                    if gps {
                        slots.push(take(Family::Gps).unwrap_or_else(|| {
                            new_slot(
                                Family::Gps,
                                bi,
                                SignalKind::L1Ca,
                                L1_HZ,
                                (1..=32).collect(),
                                600.0,
                            )
                        }));
                        searched |= System::Gps.bit();
                    }
                    if !aux_prns.is_empty() {
                        slots.push(take(Family::Aux).unwrap_or_else(|| {
                            new_slot(
                                Family::Aux,
                                bi,
                                SignalKind::L1Ca,
                                L1_HZ,
                                aux_prns.clone(),
                                if gps { 300.0 } else { 600.0 },
                            )
                        }));
                        if self.wanted(System::Qzss) {
                            searched |= System::Qzss.bit();
                        }
                        if self.wanted(System::Sbas) {
                            searched |= System::Sbas.bit();
                        }
                    }
                    if self.wanted(System::Galileo)
                        && self.fs >= 3.5e6
                        && self.cfg.galileo.is_some()
                    {
                        let n = self
                            .cfg
                            .galileo
                            .as_ref()
                            .map(|t| t.len().min(36))
                            .unwrap_or(0) as u16;
                        slots.push(take(Family::Galileo).unwrap_or_else(|| {
                            new_slot(
                                Family::Galileo,
                                bi,
                                SignalKind::E1b,
                                L1_HZ,
                                (1..=n).collect(),
                                300.0,
                            )
                        }));
                        searched |= System::Galileo.bit();
                    }
                }
                BandId::B1i => {
                    slots.push(take(Family::Beidou).unwrap_or_else(|| {
                        new_slot(
                            Family::Beidou,
                            bi,
                            SignalKind::B1i,
                            B1I_HZ,
                            (1..=codes::BDS_MAX_PRN).collect(),
                            600.0,
                        )
                    }));
                    searched |= System::Beidou.bit();
                }
                BandId::Glo(k) => {
                    slots.push(take(Family::Glo(k)).unwrap_or_else(|| {
                        new_slot(
                            Family::Glo(k),
                            bi,
                            SignalKind::G1,
                            glo_carrier(k),
                            vec![(100 + 7 + k as i16) as u16],
                            2400.0,
                        )
                    }));
                    searched |= System::Glonass.bit();
                }
            }
        }
        // The band indices can change when the bands change.
        for s in slots.iter_mut() {
            let want = match s.family {
                Family::Gps | Family::Aux | Family::Galileo => BandId::L1,
                Family::Beidou => BandId::B1i,
                Family::Glo(k) => BandId::Glo(k),
            };
            if let Some(i) = self.bands.iter().position(|b| b.id == want) {
                s.band = i;
            }
        }
        if self.wanted(System::Galileo)
            && self.cfg.galileo.is_none()
            && self.active_mask & System::Galileo.bit() != 0
        {
            let line = "Galileo E1-B codes not loaded (the galileo_codes setting): Galileo is not searched".to_string();
            if !self.log.contains(&line) {
                self.logl(line);
            }
        }
        self.slots = slots;
        self.searched_mask = searched;
    }

    /// Forgets the signal and the decoded state (a reset or a retune); the report numbers keep growing.
    pub fn reset(&mut self) {
        self.channels.clear();
        self.nav.clear();
        self.fix = None;
        self.first_fix_s = None;
        self.fix_count = 0;
        self.clock_offset = None;
        self.freq_error = None;
        self.worked_centre = None;
        self.first_lock_s = None;
        self.sky_pred.clear();
        self.prior = None;
        self.cond = Conditioner::default();
        self.bands.clear();
        self.slots.clear();
        self.pending.clear();
        self.last_used.clear();
        if self.ready {
            self.plan();
        }
        let s = self.signal_s();
        self.solver.reset(s);
        self.next_meas = s.floor() + 1.0;
        self.reset_marker = s;
    }

    /// Feeds samples of any length.
    pub fn feed(&mut self, x: &[Complex32]) {
        if !self.ready {
            return;
        }
        let mut i = 0;
        while i < x.len() {
            let take = (PIECE - self.pending.len()).min(x.len() - i);
            self.pending.extend_from_slice(&x[i..i + take]);
            i += take;
            if self.pending.len() == PIECE {
                std::mem::swap(&mut self.piece, &mut self.pending);
                self.pending.clear();
                self.run_piece();
            }
        }
    }

    /// Samples lost: the channels' phases jump, so they are lost.
    pub fn discontinuity(&mut self) {
        let mut ev = Vec::new();
        for c in self.channels.iter_mut() {
            c.drop_with(&mut ev, "samples lost (discontinuity)");
        }
        let names: Vec<String> = self.channels.iter().map(|c| c.sat.name()).collect();
        for n in names {
            self.logl(format!("{n} lost: samples lost (discontinuity)"));
        }
        for c in self.channels.iter() {
            self.good_total += c.good;
            self.bad_total += c.bad;
        }
        self.channels.clear();
    }

    fn run_piece(&mut self) {
        let fs = self.fs;
        let mut piece = std::mem::take(&mut self.piece);
        self.cond.process(&mut piece, fs);
        // Bands.
        if let Some(w) = self.wide.as_mut() {
            w.out.clear();
            w.resampler.process(&piece, &self.lut, &mut w.out);
        }
        for b in self.bands.iter_mut() {
            b.tmp.clear();
            match (b.stage1, &self.wide) {
                (Some(_), Some(w)) => b.resampler.process(&w.out, &self.lut, &mut b.tmp),
                _ => b.resampler.process(&piece, &self.lut, &mut b.tmp),
            }
            match b.excision.as_mut() {
                Some(e) => e.process(&b.tmp, &mut b.buf.data),
                None => b.buf.data.extend_from_slice(&b.tmp),
            }
        }
        self.piece = piece;
        self.pieces += 1;
        let now = self.signal_s();
        self.run_channels(now);
        self.run_searches(now);
        // Trim the bands.
        for (bi, b) in self.bands.iter_mut().enumerate() {
            let mut slowest = b.buf.end();
            for c in self.channels.iter() {
                let cb = match c.sat.system {
                    System::Beidou => BandId::B1i,
                    System::Glonass => BandId::Glo(c.glo_k()),
                    _ => BandId::L1,
                };
                if cb == b.id {
                    slowest = slowest.min(c.needs_from());
                }
            }
            let keep = if matches!(b.id, BandId::Glo(_)) {
                (1 << 17, 2048)
            } else {
                (1 << 19, 8192)
            };
            b.buf.trim(slowest.saturating_sub(keep.1), keep.0);
            let _ = bi;
        }
        // Measurement epochs covered by every band.
        loop {
            let t = self.next_meas;
            let covered = !self.bands.is_empty()
                && self
                    .bands
                    .iter()
                    .all(|b| b.buf.end() as f64 / b.rate >= t + 0.08);
            if !covered {
                break;
            }
            self.measure(t);
            self.next_meas += 1.0;
        }
        if now + 1e-9 >= self.next_report {
            self.next_report += REPORT_S;
            while self.next_report <= now {
                self.next_report += REPORT_S;
            }
            self.report(now);
            self.report_due = true;
        }
    }

    /// Returns true when channel `i` tracks the cross-correlation of a stronger L1 C/A signal: both are locked with a C/N0
    /// estimate, `i` is [`XCORR_DB`] weaker, and their Dopplers differ by a whole number of kHz within [`XCORR_HZ`].
    fn is_xcorr(&self, i: usize) -> bool {
        let c = &self.channels[i];
        if c.kind != SignalKind::L1Ca || !c.locked() || !c.cn0.ready() {
            return false;
        }
        self.channels.iter().enumerate().any(|(j, o)| {
            let d = (o.carr_freq - c.carr_freq) / 1000.0;
            j != i
                && o.kind == SignalKind::L1Ca
                && o.locked()
                && o.cn0.ready()
                && o.cn0.value - c.cn0.value >= XCORR_DB
                && (d - d.round()).abs() * 1000.0 <= XCORR_HZ
        })
    }

    fn run_channels(&mut self, now: f64) {
        self.xcorr.retain(|x| x.1 > now);
        let mut i = 0;
        while i < self.channels.len() {
            let bi = {
                let c = &self.channels[i];
                let id = match c.sat.system {
                    System::Beidou => BandId::B1i,
                    System::Glonass => BandId::Glo(c.glo_k()),
                    _ => BandId::L1,
                };
                self.bands.iter().position(|b| b.id == id)
            };
            let Some(bi) = bi else {
                self.channels.remove(i);
                continue;
            };
            let mut ev = std::mem::take(&mut self.chev);
            ev.clear();
            if self.is_xcorr(i) {
                let sat = self.channels[i].sat;
                self.channels[i].drop_with(&mut ev, "cross-correlation with a stronger satellite");
                if self.xcorr.len() < MAX_CHANNELS {
                    self.xcorr.push((sat, now + XCORR_HOLD_S));
                }
            }
            let alive = self.channels[i].run(&self.bands[bi].buf, &self.lut, now, &mut ev);
            for e in ev.drain(..) {
                self.on_chan_event(i, e, now);
            }
            self.chev = ev;
            if !alive {
                let c = self.channels.remove(i);
                self.good_total += c.good;
                self.bad_total += c.bad;
                // A lost channel locked over 0.5 s during a long round ends that round.
                if c.was_locked {
                    for s in self.slots.iter_mut() {
                        if s.band == bi && s.window.2 >= 64 {
                            s.pos = s.order.len();
                        }
                    }
                }
                continue;
            }
            i += 1;
        }
    }

    fn on_chan_event(&mut self, i: usize, e: ChanEvent, now: f64) {
        let name = self.channels[i].sat.name();
        match e {
            ChanEvent::Locked => {
                let c = &self.channels[i];
                let line = format!("{name} locked, C/N0 {:.1} dB-Hz", c.cn0.value);
                if self.first_lock_s.is_none() {
                    self.first_lock_s = Some(now);
                }
                if self.worked_centre.is_none() && c.kind == SignalKind::L1Ca {
                    self.worked_centre = Some(c.carr_freq);
                }
                self.logl(line);
            }
            ChanEvent::Log(l) => self.logl(format!("{name}: {l}")),
            ChanEvent::Lost { reason, confirmed } => {
                if confirmed {
                    self.logl(format!("{name} lost: {reason}"));
                } else {
                    self.logl(format!("{name} not confirmed: {reason}"));
                }
            }
            ChanEvent::Bad => {}
            ChanEvent::Unit {
                unit,
                start_epoch,
                time,
            } => {
                let c = &self.channels[i];
                let sat = c.sat;
                let glo_k = c.glo_k();
                if let NavUnit::Sbas { bits, msg_type } = &unit {
                    self.sbas_total += 1;
                    let m = SbasMessage {
                        prn: sat.prn,
                        bits: bits.clone(),
                        msg_type: *msg_type,
                        signal_s: now,
                        cn0: c.cn0.value,
                        doppler: c.carr_freq,
                    };
                    self.out_sbas.push((now, m));
                }
                self.out_frames.push((
                    now,
                    NavFrame {
                        sat,
                        unit: unit.clone(),
                        time,
                        start_epoch,
                    },
                ));
                let news = self.nav.add(sat, glo_k, &unit, time, now);
                for n in news {
                    match n {
                        NavNews::Log(l) => self.logl(l),
                        NavNews::Ephemeris(id, e) => self.out_eph.push((now, (id, e))),
                        NavNews::GloSlot(k, slot) => {
                            if let Some(ch) = self
                                .channels
                                .iter_mut()
                                .find(|c| c.sat.system == System::Glonass && c.glo_k() == k)
                            {
                                ch.sat = SatId::new(System::Glonass, slot);
                                ch.set_glo_k(k);
                            }
                        }
                    }
                }
                // The ephemeris level.
                let c = &mut self.channels[i];
                let has =
                    self.nav.eph.contains_key(&c.sat) || self.nav.glo_eph.contains_key(&c.sat);
                if has && c.state == ChannelState::FrameSync {
                    c.state = ChannelState::Ephemeris;
                }
            }
        }
    }

    // ------------------------------------------------------------------ searches

    fn locked_on_band(&self, band: usize) -> bool {
        let id = self.bands[band].id;
        self.channels.iter().any(|c| {
            c.locked()
                && match c.sat.system {
                    System::Beidou => id == BandId::B1i,
                    System::Glonass => id == BandId::Glo(c.glo_k()),
                    _ => id == BandId::L1,
                }
        })
    }

    /// Returns true when the satellite `prn` of `fam` has a channel, or waits after a cross-correlation.
    fn tracked(&self, fam: Family, prn: u16) -> bool {
        let held = self.xcorr.iter().any(|(sat, _)| match fam {
            Family::Gps => *sat == SatId::new(System::Gps, prn),
            Family::Aux => System::of_l1_prn(prn).is_some_and(|s| *sat == SatId::new(s, prn)),
            _ => false,
        });
        held || self.channels.iter().any(|c| match fam {
            Family::Gps => c.sat == SatId::new(System::Gps, prn),
            Family::Aux => System::of_l1_prn(prn).is_some_and(|s| c.sat == SatId::new(s, prn)),
            Family::Galileo => c.sat == SatId::new(System::Galileo, prn),
            Family::Beidou => c.sat == SatId::new(System::Beidou, prn),
            Family::Glo(k) => c.sat.system == System::Glonass && c.glo_k() == k,
        })
    }

    fn gps_locked(&self) -> bool {
        self.channels
            .iter()
            .any(|c| c.sat.system == System::Gps && c.locked())
    }

    /// The window (centre Hz, half width Hz, periods) of a family, scaled to its carrier.
    fn window(&self, si: usize) -> (f64, f64, usize) {
        let s = &self.slots[si];
        let scale = s.carrier / L1_HZ;
        let stage_plan = |stage: u8, centre: f64| -> (f64, f64, usize) {
            let half = [self.cfg.search_half_width, 45e3, 170e3][(stage % 3) as usize];
            let k = if stage >= 3 { 64 } else { 16 };
            (centre, half, k)
        };
        let base_centre = self.worked_centre.or(self.cfg.freq_hint).unwrap_or(0.0);
        let gps_window = || -> (f64, f64, usize) {
            if let (Some(_), Some(fe)) = (&self.fix, self.freq_error) {
                return (fe, 6.5e3, 16);
            }
            let span = span(
                self.channels
                    .iter()
                    .filter(|c| c.locked() && c.kind == SignalKind::L1Ca)
                    .map(|c| c.carr_freq),
            );
            if let Some((lo, hi)) = span {
                let half = (11e3 - (hi - lo) / 2.0).max(5.5e3) + 0.5e3;
                return (0.5 * (lo + hi), half, 16);
            }
            let gps_stage = self
                .slots
                .iter()
                .find(|x| x.family == Family::Gps)
                .map(|x| x.stage)
                .unwrap_or(0);
            stage_plan(gps_stage, base_centre)
        };
        match s.family {
            Family::Gps => {
                let mut w = gps_window();
                if w.1 < 15e3 && s.long_round && self.fix.is_none() {
                    w.2 = 64;
                }
                w
            }
            Family::Aux => {
                if self.cfg.sbas_mode || !self.wanted(System::Gps) {
                    let span = span(
                        self.channels
                            .iter()
                            .filter(|c| c.locked())
                            .map(|c| c.carr_freq),
                    );
                    if let Some((lo, hi)) = span {
                        return (0.5 * (lo + hi), 6e3, 16);
                    }
                    return stage_plan(s.stage, base_centre);
                }
                let w = gps_window();
                let k = if s.round % 2 == 1 && s.prev_found {
                    16
                } else {
                    8
                };
                (w.0, w.1, k)
            }
            Family::Galileo => {
                let fe = self.freq_error.unwrap_or(base_centre);
                let tracking = self
                    .channels
                    .iter()
                    .any(|c| c.sat.system == System::Galileo);
                let k = if tracking && s.round % 3 == 2 { 12 } else { 4 };
                (fe, 5e3, k)
            }
            Family::Beidou | Family::Glo(_) => {
                let with_l1 = self.bands.iter().any(|b| b.id == BandId::L1);
                if with_l1 {
                    if let (Some(_), Some(fe)) = (&self.fix, self.freq_error) {
                        let tracking = self.channels.iter().any(|c| c.carrier_hz == s.carrier);
                        let k = if s.family == Family::Beidou && tracking && s.round % 3 == 2 {
                            32
                        } else {
                            16
                        };
                        return (fe * scale, 5.5e3, k);
                    }
                    let w = gps_window();
                    return (w.0 * scale, w.1, 16);
                }
                // On its own tune: the widening plan (GLONASS: driven from the first channel).
                let stage = if let Family::Glo(_) = s.family {
                    self.slots
                        .iter()
                        .find(|x| matches!(x.family, Family::Glo(_)))
                        .map(|x| x.stage)
                        .unwrap_or(0)
                } else {
                    s.stage
                };
                let span = span(
                    self.channels
                        .iter()
                        .filter(|c| {
                            c.locked()
                                && c.sat.system
                                    == if s.family == Family::Beidou {
                                        System::Beidou
                                    } else {
                                        System::Glonass
                                    }
                        })
                        .map(|c| c.carr_freq / c.carrier_hz * L1_HZ),
                );
                if let Some((lo, hi)) = span {
                    let half = (11e3 - (hi - lo) / 2.0).max(5.5e3) + 0.5e3;
                    return (0.5 * (lo + hi) * scale, half, 16);
                }
                let (c, h, k) = stage_plan(stage, base_centre);
                (c * scale, h, k)
            }
        }
    }

    /// The index of the `k`-th GLONASS slot.
    fn nth_glo_slot(&self, k: usize) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.family, Family::Glo(_)))
            .nth(k)
            .map(|(i, _)| i)
    }

    fn slot_active(&self, si: usize, now: f64) -> bool {
        let s = &self.slots[si];
        if now < s.sleep_until {
            return false;
        }
        let with_l1 = self.bands.iter().any(|b| b.id == BandId::L1);
        match s.family {
            Family::Gps => true,
            Family::Aux => self.cfg.sbas_mode || !self.wanted(System::Gps) || self.gps_locked(),
            Family::Galileo => {
                self.fix.is_some()
                    && self.freq_error.is_some()
                    && self.channels.iter().any(|c| {
                        c.sat.system == System::Gps
                            && c.locked()
                            && c.locked_epochs as f64 * 1e-3 > 3.0
                    })
            }
            Family::Beidou | Family::Glo(_) => {
                !with_l1 || self.gps_locked() || !self.wanted(System::Gps)
            }
        }
    }

    /// The code-rate hint: the median code-rate error of the channels locked over 3 s.
    fn code_hint(&self) -> f64 {
        let mut v = [0.0; MAX_CHANNELS];
        let mut n = 0;
        let long = self
            .channels
            .iter()
            .filter(|c| c.locked() && c.locked_epochs as f64 * c.period() > 3.0);
        for (slot, c) in v.iter_mut().zip(long) {
            *slot = c.code_rate_err();
            n += 1;
        }
        crate::fixsolve::median(&mut v[..n]).unwrap_or(0.0)
    }

    fn run_searches(&mut self, now: f64) {
        let piece_ms = PIECE as f64 / self.fs * 1e3;
        let base = self.cfg.effort * piece_ms;
        let with_l1 = self.bands.iter().any(|b| b.id == BandId::L1);
        let gps_on = self.slots.iter().any(|s| s.family == Family::Gps);
        // Budgets.
        let nglo = self
            .slots
            .iter()
            .filter(|s| matches!(s.family, Family::Glo(_)))
            .count();
        let mut glo_pool = 0.0;
        for si in 0..self.slots.len() {
            let fam = self.slots[si].family;
            let band = self.slots[si].band;
            let idle = !self.locked_on_band(band);
            let mut b = match fam {
                Family::Gps => base,
                Family::Aux => {
                    if gps_on {
                        base / 2.0
                    } else {
                        base
                    }
                }
                Family::Galileo => base / 3.0,
                Family::Beidou => {
                    if with_l1 && gps_on {
                        base / 2.0
                    } else {
                        base
                    }
                }
                Family::Glo(_) => 0.0,
            };
            if idle {
                b *= 2.0;
            }
            if let Family::Glo(_) = fam {
                let first_glo = self
                    .slots
                    .iter()
                    .position(|s| matches!(s.family, Family::Glo(_)));
                if first_glo == Some(si) {
                    let mut g = if with_l1 && gps_on { base / 2.0 } else { base };
                    if !self
                        .channels
                        .iter()
                        .any(|c| c.sat.system == System::Glonass && c.locked())
                    {
                        g *= 2.0;
                    }
                    glo_pool = g;
                }
                continue;
            }
            let s = &mut self.slots[si];
            s.budget = (s.budget + b).min(s.cap);
        }
        if nglo > 0 {
            let share = (glo_pool / nglo as f64).max(8.0);
            let start = self.glo_rot % nglo;
            self.glo_rot += 1;
            let mut pool = glo_pool;
            for j in 0..nglo {
                if pool <= 0.0 {
                    break;
                }
                let Some(si) = self.nth_glo_slot((start + j) % nglo) else {
                    break;
                };
                if let Family::Glo(k) = self.slots[si].family {
                    if self.tracked(Family::Glo(k), 0) {
                        continue;
                    }
                }
                let give = share.min(pool);
                let s = &mut self.slots[si];
                s.budget = (s.budget + give).min(s.cap);
                pool -= give;
            }
        }
        for si in 0..self.slots.len() {
            if !self.slot_active(si, now) {
                continue;
            }
            self.step_slot(si, now);
        }
    }

    fn step_slot(&mut self, si: usize, now: f64) {
        loop {
            if self.slots[si].budget < 8.0 {
                return;
            }
            if self.slots[si].job.is_none() && !self.start_next(si, now) {
                return;
            }
            let bi = self.slots[si].band;
            let s = &mut self.slots[si];
            let Some(mut job) = s.job.take() else { return };
            let plot = self.plot.get_or_insert_with(AcqPlot::default);
            match s.searcher.step_into(&mut job, &mut s.budget, plot) {
                None => {
                    s.job = Some(job);
                    return;
                }
                Some(hit) => {
                    s.spare = Some(job);
                    self.on_result(si, bi, hit, now);
                }
            }
        }
    }

    /// Starts the next satellite of the round (or the next round). Returns false when it must wait for samples.
    fn start_next(&mut self, si: usize, now: f64) -> bool {
        if self.channels.len() >= MAX_CHANNELS {
            return false;
        }
        let window = self.window(si);
        let hint = self.code_hint();
        let bi = self.slots[si].band;
        let band_end = self.bands[bi].buf.end();
        let fam = self.slots[si].family;
        // Next satellite not tracked. If every satellite of six rounds in a row is tracked, wait for the next piece.
        let mut rounds = 0;
        loop {
            let s = &self.slots[si];
            if s.pos >= s.order.len() {
                if rounds == MAX_EMPTY_ROUNDS {
                    return false;
                }
                rounds += 1;
                self.end_round(si, now);
                let s = &self.slots[si];
                if now < s.sleep_until {
                    return false;
                }
            }
            let s = &self.slots[si];
            if s.pos < s.order.len() {
                let prn = s.order[s.pos];
                if self.tracked(fam, prn) {
                    self.slots[si].pos += 1;
                    continue;
                }
                break;
            }
        }
        let s = &mut self.slots[si];
        s.window = window;
        let k = window.2;
        let n = s.kind.period_samples();
        let need = (k * n) as u64;
        if band_end < need || band_end - need < s.last_seg_end {
            return false;
        }
        let seg_start = band_end - need;
        let prn = s.order[s.pos];
        let Some(seg) = self.bands[bi].buf.slice(seg_start, k * n) else {
            return false;
        };
        if !s.searcher.replicas.has(prn) {
            let chips = match fam {
                Family::Galileo => self
                    .cfg
                    .galileo
                    .as_ref()
                    .and_then(|t| t.code(prn))
                    .map(|c| c.to_vec()),
                Family::Beidou => codes::bds_b1i(prn),
                Family::Glo(_) => Some(codes::glonass_code()),
                _ => codes::l1_ca(prn),
            };
            let Some(chips) = chips else {
                s.pos += 1;
                return true;
            };
            s.searcher.add_code(prn, &chips);
        }
        let integ = Integration {
            periods: k,
            coherent: if s.kind == SignalKind::L1Ca { 2 } else { 1 },
        };
        let subs = if s.kind == SignalKind::E1b { 2 } else { 4 };
        let job = s.searcher.start(
            prn,
            seg,
            seg_start,
            window.0,
            window.1,
            subs,
            integ,
            s.carrier,
            hint,
            1e-3,
            s.spare.take(),
        );
        s.job = Some(job);
        s.current = Some(prn);
        s.last_seg_end = band_end;
        true
    }

    fn end_round(&mut self, si: usize, now: f64) {
        let pulling = self.channels.iter().any(|c| !c.locked());
        let tracked_any = self.channels.iter().any(|c| c.locked());
        let fam = self.slots[si].family;
        let mut order = std::mem::take(&mut self.slots[si].order);
        self.round_order(si, &mut order);
        let s = &mut self.slots[si];
        let found = s.found_round;
        s.round += 1;
        s.prev_found = found;
        s.found_round = false;
        s.pos = 0;
        s.order = order;
        s.long_round = !found && !s.long_round;
        let widening = match fam {
            Family::Gps => true,
            Family::Aux => self.cfg.sbas_mode,
            Family::Beidou | Family::Glo(_) => !self.bands.iter().any(|b| b.id == BandId::L1),
            Family::Galileo => false,
        };
        if widening && !found && !pulling && !tracked_any {
            s.stage = (s.stage + 1) % 6;
            let stage = s.stage;
            let what = match stage % 3 {
                0 => format!("±{:.0} kHz", self.cfg.search_half_width / 1e3),
                1 => "±45 kHz".into(),
                _ => "±170 kHz".into(),
            };
            let ms = if stage >= 3 { 64 } else { 16 };
            self.logl(format!(
                "nothing found yet: searching {what} with {ms} ms (stage {stage})"
            ));
        }
        let s = &mut self.slots[si];
        let sleeper = matches!(fam, Family::Aux | Family::Galileo)
            || (matches!(fam, Family::Beidou | Family::Glo(_))
                && self.bands.iter().any(|b| b.id == BandId::L1));
        let sleeper = sleeper && !(fam == Family::Aux && self.cfg.sbas_mode);
        let none_tracked = !self.channels.iter().any(|c| match fam {
            Family::Aux => matches!(c.sat.system, System::Qzss | System::Sbas),
            Family::Galileo => c.sat.system == System::Galileo,
            Family::Beidou => c.sat.system == System::Beidou,
            Family::Glo(_) => c.sat.system == System::Glonass,
            Family::Gps => false,
        });
        if sleeper && !found && none_tracked && s.round > 1 {
            s.sleep_len = if s.sleep_len == 0.0 {
                10.0
            } else {
                (s.sleep_len * 2.0).min(80.0)
            };
            s.sleep_until = now + s.sleep_len;
        } else if found {
            s.sleep_len = 0.0;
        }
    }

    /// Writes the order of the next round of slot `si` into `v`: the predicted satellites first, then the others.
    fn round_order(&self, si: usize, v: &mut Vec<u16>) {
        let s = &self.slots[si];
        v.clear();
        let sys = match s.family {
            Family::Gps => Some(System::Gps),
            Family::Galileo => Some(System::Galileo),
            Family::Beidou => Some(System::Beidou),
            _ => None,
        };
        if let Some(sys) = sys {
            for id in &self.sky_pred {
                if id.system == sys && s.prns.contains(&id.prn) {
                    v.push(id.prn);
                }
            }
        }
        // GPS with a fix: only the predicted ones, except every sixth round.
        if s.family == Family::Gps
            && !v.is_empty()
            && self.fix.is_some()
            && !(s.round + 1).is_multiple_of(6)
        {
            return;
        }
        for &p in &s.prns {
            if !v.contains(&p) {
                v.push(p);
            }
        }
    }

    fn on_result(&mut self, si: usize, bi: usize, hit: AcqHit, now: f64) {
        let fam = self.slots[si].family;
        if let Some(first) = self.slots[si].confirming.take() {
            // The confirmation.
            self.slots[si].pos += 1;
            if !hit.found {
                return;
            }
            let s = &self.slots[si];
            let n = s.kind.period_samples() as f64;
            // Code-rate error from the two readings.
            let mut code_rate = 0.0;
            if matches!(fam, Family::Beidou | Family::Glo(_)) {
                let d = (hit.seg_start as f64 + hit.code_phase)
                    - (first.seg_start as f64 + first.code_phase);
                let per = n / (1.0 + hit.doppler_hz / s.carrier);
                let m = (d / per).round();
                if m > 0.0 {
                    let frac = (d - m * per) / (m * per);
                    if frac.abs() < 1e-4 {
                        code_rate = -frac;
                    }
                }
            }
            let prn = hit.prn;
            let sat = match fam {
                Family::Gps => SatId::new(System::Gps, prn),
                Family::Aux => SatId::new(System::of_l1_prn(prn).unwrap_or(System::Sbas), prn),
                Family::Galileo => SatId::new(System::Galileo, prn),
                Family::Beidou => SatId::new(System::Beidou, prn),
                Family::Glo(k) => SatId::glo_channel(k),
            };
            if self.channels.iter().any(|c| c.sat == sat) || self.channels.len() >= MAX_CHANNELS {
                return;
            }
            let chips = match fam {
                Family::Galileo => self
                    .cfg
                    .galileo
                    .as_ref()
                    .and_then(|t| t.code(prn))
                    .map(|c| c.to_vec())
                    .unwrap_or_default(),
                Family::Beidou => codes::bds_b1i(prn).unwrap_or_default(),
                Family::Glo(_) => codes::glonass_code(),
                _ => codes::l1_ca(prn).unwrap_or_default(),
            };
            let hint = self.code_hint() + code_rate;
            let mut ch = Channel::start(
                sat,
                s.kind,
                s.carrier,
                &chips,
                &hit,
                self.bands[bi].buf.end(),
                hint,
                0.0,
                now,
            );
            if let Family::Glo(k) = fam {
                ch.set_glo_k(k);
            }
            self.channels.push(ch);
            return;
        }
        if !hit.found {
            self.slots[si].pos += 1;
            return;
        }
        // A hit: log it and confirm it on the newest samples.
        let name = match fam {
            Family::Glo(k) => SatId::glo_channel(k).name(),
            Family::Aux => {
                SatId::new(System::of_l1_prn(hit.prn).unwrap_or(System::Sbas), hit.prn).name()
            }
            Family::Gps => SatId::new(System::Gps, hit.prn).name(),
            Family::Galileo => SatId::new(System::Galileo, hit.prn).name(),
            Family::Beidou => SatId::new(System::Beidou, hit.prn).name(),
        };
        self.logl(format!(
            "{name} found: Doppler {:+.0} Hz, peak {:.1} times the noise",
            hit.doppler_hz, hit.ratio
        ));
        self.out_hits.push((now, hit.clone()));
        let s = &mut self.slots[si];
        s.found_round = true;
        let k = s.window.2.max(8);
        let n = s.kind.period_samples();
        let end = self.bands[bi].buf.end();
        let seg_start = end.saturating_sub((k * n) as u64);
        let Some(seg) = self.bands[bi].buf.slice(seg_start, k * n) else {
            s.pos += 1;
            return;
        };
        let integ = Integration {
            periods: k,
            coherent: if s.kind == SignalKind::L1Ca { 2 } else { 1 },
        };
        let hint = 0.0;
        let job = s.searcher.start_confirm(
            hit.prn,
            seg,
            seg_start,
            hit.doppler_hz,
            integ,
            s.carrier,
            hint,
            s.spare.take(),
        );
        s.job = Some(job);
        s.confirming = Some(hit);
    }

    // ------------------------------------------------------------------ measurements and the fix

    fn measure(&mut self, t: f64) {
        let mut ep = MeasEpoch {
            rx_time: t,
            sats: Vec::new(),
            gps_time: self.clock_offset.map(|o| {
                let g = t + o;
                let w = (g / WEEK_S).floor();
                (w as i64, g - w * WEEK_S)
            }),
        };
        let mut chans: Vec<(usize, f64)> = Vec::new();
        for (ci, c) in self.channels.iter().enumerate() {
            if !c.locked() || c.sat.system == System::Sbas {
                continue;
            }
            let rate = c.kind.band_rate();
            let Some(tx) = c.transmit_time_at(t * rate) else {
                continue;
            };
            chans.push((ci, tx));
            ep.sats.push(SatMeas {
                sat: c.sat,
                glo_k: (c.sat.system == System::Glonass).then(|| c.glo_k()),
                tx_time: tx,
                doppler: c.carr_freq,
                carrier: c.carrier_hz,
                cn0: c.cn0.value,
                used: false,
                residual: None,
            });
        }
        let sol = self.solve(&ep, t);
        if let Some((used, resid)) = &sol {
            for m in ep.sats.iter_mut() {
                if let Some(k) = used.iter().position(|u| *u == m.sat) {
                    m.used = true;
                    m.residual = Some(resid[k]);
                }
            }
        }
        if self.fix.is_some() && t - self.last_fix_s > 20.0 {
            self.fix = None;
            self.logl("fix lost: no solution for 20 s");
        }
        self.out_meas.push((t, ep));
        // Clock-comparison and histories, one point a second.
        push_cap(&mut self.level_hist, self.cond.level_dbfs() as f32, 120);
        let best = self
            .channels
            .iter()
            .filter(|c| c.locked())
            .map(|c| c.cn0.value)
            .fold(0.0, f64::max);
        push_cap(&mut self.cn0_hist, best as f32, 120);
        let off = self.clock_offsets.back().copied();
        push_cap(
            &mut self.clock_hist,
            off.map(|v| v as f32).unwrap_or(f32::NAN),
            120,
        );
    }

    /// Solves the fix of a measurement epoch; returns the used satellites and their residuals.
    fn solve(&mut self, ep: &MeasEpoch, t: f64) -> Option<(Vec<SatId>, Vec<f64>)> {
        self.solver.mask_deg = self.cfg.elevation_mask;
        let fix = self.solver.solve(&self.nav, ep, t)?;
        self.freq_error = self.solver.freq_error;
        self.clock_offset = self.solver.clock_offset;
        self.first_fix_s = self.solver.first_fix_s;
        self.fix_count = self.solver.fix_count;
        self.prior = self.solver.prior;
        self.last_used = self.solver.last_used.clone();
        if self.fix_count == 1 {
            self.logl(format!(
                "first fix at {:.1} s: {:.6} {:.6}, height {:.1} m, {} satellites, HDOP {:.1}",
                fix.first_fix_s, fix.lat, fix.lon, fix.height, fix.satellites, fix.hdop
            ));
        }
        self.last_fix_s = t;
        if self.fix_count % 10 == 1 {
            let e = FIX.event(
                t,
                self.centre,
                &GnssFixEvent {
                    family: keys::Family::Receiver,
                    position: (fix.lat, fix.lon),
                    height_m: fix.height,
                    speed_kt: fix.has_velocity.then_some(fix.speed * MS_TO_KT),
                    course_deg: fix.course.filter(|_| fix.has_velocity),
                    satellites_used: fix.satellites as i64,
                },
            );
            self.events.push(e);
        }
        self.predict_sky(&fix);
        self.out_fix.push((t, fix.clone()));
        let used: Vec<SatId> = self.last_used.keys().copied().collect();
        let res: Vec<f64> = self.last_used.values().map(|v| v.0).collect();
        self.fix = Some(fix);
        Some((used, res))
    }

    fn predict_sky(&mut self, fix: &Fix) {
        let geo = [fix.lat.to_radians(), fix.lon.to_radians(), fix.height];
        let mut v = Vec::new();
        for (id, e) in &self.nav.eph {
            let p = e.position(
                fix.tow
                    - if id.system == System::Beidou {
                        14.0
                    } else {
                        0.0
                    },
            );
            if az_el(geo, fix.ecef, p).1 > (-2f64).to_radians() {
                v.push(*id);
            }
        }
        for (prn, a) in &self.nav.almanac {
            let id = SatId::new(System::Gps, *prn);
            if v.contains(&id) {
                continue;
            }
            if az_el(geo, fix.ecef, a.position(fix.tow)).1 > (-2f64).to_radians() {
                v.push(id);
            }
        }
        self.sky_pred = v;
    }

    // ------------------------------------------------------------------ report

    fn report(&mut self, now: f64) {
        let fix_geo = self.fix.as_ref().map(|f| {
            (
                [f.lat.to_radians(), f.lon.to_radians(), f.height],
                f.ecef,
                f.tow,
            )
        });
        let mut rows = Vec::new();
        let mut order: Vec<usize> = (0..self.channels.len()).collect();
        order.sort_by(|&a, &b| {
            self.channels[b]
                .cn0
                .value
                .total_cmp(&self.channels[a].cn0.value)
        });
        for &i in order.iter().take(MAX_CHANNELS) {
            let c = &self.channels[i];
            let used = self.last_used.get(&c.sat);
            let (az, el) = match (fix_geo, self.nav.eph.get(&c.sat)) {
                (Some((g, p, tow)), Some(e)) => {
                    let t = tow
                        - if c.sat.system == System::Beidou {
                            14.0
                        } else {
                            0.0
                        };
                    let (a, el) = az_el(g, p, e.position(t));
                    (Some(a.to_degrees()), Some(el.to_degrees()))
                }
                _ => match used {
                    Some(u) => (Some(u.1.to_degrees()), Some(u.2.to_degrees())),
                    None => (None, None),
                },
            };
            let health = self.nav.healthy(c.sat);
            let pos = self
                .bands
                .iter()
                .find(|b| b.rate == c.kind.band_rate())
                .map(|b| b.buf.end() as f64)
                .unwrap_or(0.0);
            let chips = c.code_chips(pos);
            rows.push(ChannelRow {
                sat: Some(c.sat),
                name: c.sat.name(),
                system: Some(c.sat.system),
                glo_k: (c.sat.system == System::Glonass).then(|| c.glo_k()),
                cn0: c.cn0.value,
                doppler: c.carr_freq,
                code_chips: if c.kind == SignalKind::E1b {
                    chips.floor()
                } else {
                    chips
                },
                az,
                el,
                state: Some(c.state),
                state_level: c.state as u8,
                lock_s: c.lock_time_s.map(|t| now - t).unwrap_or(0.0),
                good: c.good,
                bad: c.bad,
                residual: used.map(|u| u.0),
                health,
                in_fix: used.is_some(),
                code_freq: c.code_freq(),
                cn0_history: c.cn0_history.iter().copied().collect(),
            });
        }
        self.out_channels.push((now, rows.clone()));
        // Navigation rows.
        let mut nav_rows = Vec::new();
        for r in &rows {
            let Some(sat) = r.sat else { continue };
            let n = self.nav.sats.get(&sat);
            let mut row = NavRow {
                name: r.name.clone(),
                tow: n.and_then(|n| n.last_tow),
                ..Default::default()
            };
            let gps_t = self.fix.as_ref().map(|f| f.tow);
            if sat.system == System::Sbas {
                row.ephemeris = Some(EphemerisState::NotUsed);
            } else if let Some(e) = self.nav.eph.get(&sat) {
                row.ephemeris = Some(EphemerisState::Held);
                row.iod = Some(i64::from(e.iode));
                row.week = Some(e.week);
                row.af0_us = Some(e.af0 * 1e6);
                row.age_s = gps_t.map(|t| {
                    e.age(
                        t - if sat.system == System::Beidou {
                            14.0
                        } else {
                            0.0
                        },
                    )
                });
            } else if let Some(e) = self.nav.glo_eph.get(&sat) {
                row.ephemeris = Some(EphemerisState::Held);
                row.iod = Some((e.tb / 900.0).round() as i64);
                row.af0_us = Some(-e.tau_n * 1e6);
            } else {
                let (p, of) = self.nav.parts(sat);
                if p == 0 {
                    row.ephemeris = Some(EphemerisState::Waiting);
                } else {
                    row.ephemeris = Some(EphemerisState::Collecting);
                    row.parts = Some((p as u32, of as u32));
                    row.seconds_left = self.nav.seconds_left(sat, now);
                }
            }
            nav_rows.push(row);
        }
        // Sky and sub-points.
        let (sky, subs) = self.sky_and_points(&rows);
        let state = if self.fix.is_some() {
            2
        } else if self.channels.iter().any(|c| c.locked()) {
            1
        } else {
            0
        };
        // Search info: the first running slot.
        let mut si = SearchInfo {
            first_lock_s: self.first_lock_s,
            pulling_in: self.channels.iter().filter(|c| !c.locked()).count(),
            work_units: self.slots.iter().map(|s| s.searcher.work()).sum(),
            ..Default::default()
        };
        if let Some(s) = self.slots.iter().find(|s| s.job.is_some()) {
            si.running = true;
            si.system = Some(s.family.system(s.current.unwrap_or(0)));
            si.prn = s.current.unwrap_or(0);
            let per = s.pos as f64 + s.job.as_ref().map(|j| j.progress()).unwrap_or(0.0);
            si.progress = per / s.order.len().max(1) as f64;
            si.rounds = s.round;
            si.centre_hz = s.window.0;
            si.half_hz = s.window.1;
            si.ms = s.window.2 as f64 * s.kind.period_s() * 1e3;
            si.stage = if state > 0 { 0 } else { s.stage };
        } else if let Some(s) = self.slots.first() {
            si.rounds = s.round;
            si.stage = if state > 0 { 0 } else { s.stage };
        }
        let mut scatter = Vec::new();
        let mut scatter_sat = String::new();
        if let Some(c) = self
            .channels
            .iter()
            .filter(|c| c.locked() && c.scatter.len() >= 20)
            .max_by(|a, b| a.cn0.value.total_cmp(&b.cn0.value))
        {
            let mx = c
                .scatter
                .iter()
                .map(|p| (p[0] * p[0] + p[1] * p[1]).sqrt())
                .fold(1e-9, f32::max);
            scatter = c.scatter.iter().map(|p| [p[0] / mx, p[1] / mx]).collect();
            scatter_sat = c.sat.name();
        }
        let best = rows
            .iter()
            .filter(|r| r.state_level >= 1)
            .map(|r| r.cn0)
            .fold(0.0, f64::max);
        let tracked = rows.iter().filter(|r| r.state_level >= 1).count();
        let status_line = match &self.fix {
            Some(f) => format!(
                "{}, {:.6} {:.6}, height {:.0} m, HDOP {:.1}, best {:.0} dB-Hz",
                f.type_text, f.lat, f.lon, f.height, f.hdop, best
            ),
            None if tracked > 0 => {
                format!("tracking {tracked} satellites, best {best:.0} dB-Hz, no fix yet")
            }
            None => format!(
                "searching ({})",
                if let (true, Some(sys)) = (si.running, si.system) {
                    format!("{} {}", sys.name(), si.prn)
                } else {
                    "waiting".into()
                }
            ),
        };
        let progress = self.progress_lines(&si, now);
        let leap_ann = self
            .nav
            .utc
            .filter(|u| u.dtlsf != u.dtls)
            .map(|u| (u.dtlsf, u.wnlsf, u.dn));
        let avg = (!self.clock_offsets.is_empty())
            .then(|| self.clock_offsets.iter().sum::<f64>() / self.clock_offsets.len() as f64);
        self.report = GnssReport {
            input_rate: self.fs,
            centre_hz: self.centre,
            wanted_mask: self.cfg.systems,
            active_mask: self.active_mask,
            searched_mask: self.searched_mask
                | self.channels.iter().fold(0, |m, c| m | c.sat.system.bit()),
            signal_s: now,
            state,
            channels: rows,
            sky,
            satellites: subs,
            navigation: nav_rows,
            fix: self.fix.clone(),
            iono_valid: self.nav.iono.is_some(),
            iono: self.nav.iono,
            utc_valid: self.nav.utc.is_some(),
            utc: self.nav.utc,
            leap_announcement: leap_ann,
            clock_offset_ms: self.clock_offsets.back().copied(),
            clock_offset_avg_ms: avg,
            level_history: self.level_hist.iter().copied().collect(),
            cn0_history: self.cn0_hist.iter().copied().collect(),
            clock_history: self.clock_hist.iter().copied().collect(),
            almanac_count: self.nav.almanac.len(),
            sbas_total: self.sbas_total,
            search: si,
            acq_plot: self.plot.clone(),
            scatter,
            scatter_sat,
            front_end: FrontEndInfo {
                level_dbfs: self.cond.level_dbfs(),
                clip_percent: self.cond.clip_percent(),
                dc_i: self.cond.dc.0,
                dc_q: self.cond.dc.1,
                bins_cut: self
                    .bands
                    .iter()
                    .filter_map(|b| b.excision.as_ref())
                    .map(|e| e.bins_cut)
                    .sum(),
            },
            status_line,
            progress,
            frequency_error_hz: self.freq_error,
            no_band: self.no_band,
        };
    }

    fn sky_and_points(&self, rows: &[ChannelRow]) -> (Vec<SkyEntry>, Vec<SubPoint>) {
        let mut sky = Vec::new();
        let mut subs = Vec::new();
        let tracked = |id: SatId| rows.iter().find(|r| r.sat == Some(id));
        let gps_tow = self.fix.as_ref().map(|f| f.tow).or_else(|| {
            self.nav
                .sats
                .iter()
                .filter(|(k, _)| k.system == System::Gps)
                .filter_map(|(_, n)| n.last_tow)
                .fold(None, |a: Option<f64>, b| Some(a.map_or(b, |a| a.max(b))))
        });
        let Some(tow) = gps_tow else {
            return (sky, subs);
        };
        let mut add = |id: SatId, p: [f64; 3], from_eph: bool, age: f64, healthy: bool| {
            let tr = tracked(id);
            let used = self.last_used.contains_key(&id);
            if let Some(f) = &self.fix {
                let g = [f.lat.to_radians(), f.lon.to_radians(), f.height];
                let (az, el) = az_el(g, f.ecef, p);
                if el > 0.0 && sky.len() < 64 {
                    sky.push(SkyEntry {
                        system: Some(id.system),
                        prn: id.prn,
                        name: id.name(),
                        glo_k: tr.and_then(|r| r.glo_k),
                        az: az.to_degrees(),
                        el: el.to_degrees(),
                        tracked: tr.is_some(),
                        used,
                        cn0: tr.map(|r| r.cn0).unwrap_or(0.0),
                        healthy,
                        from_ephemeris: from_eph,
                    });
                }
            }
            let g = ecef_to_geodetic(p);
            let hkm = g[2] / 1e3;
            if hkm >= 100.0 && g.iter().all(|v| v.is_finite()) && subs.len() < 96 {
                let valid = from_eph
                    && age.abs()
                        <= if id.system == System::Glonass {
                            1800.0
                        } else {
                            4.0 * 3600.0
                        };
                subs.push(SubPoint {
                    name: id.name(),
                    system: Some(id.system),
                    lat: g[0].to_degrees(),
                    lon: g[1].to_degrees(),
                    height_km: hkm,
                    tracked: tr.is_some(),
                    used,
                    cn0: tr.map(|r| r.cn0).unwrap_or(0.0),
                    healthy,
                    from_ephemeris: from_eph,
                    age_s: if from_eph { age } else { -1.0 },
                    public: valid && id.glo_k().is_none(),
                });
            }
        };
        for (id, e) in &self.nav.eph {
            let t = tow
                - if id.system == System::Beidou {
                    14.0
                } else {
                    0.0
                };
            add(*id, e.position(t), true, e.age(t), e.health == 0);
        }
        for (prn, a) in &self.nav.almanac {
            let id = SatId::new(System::Gps, *prn);
            if self.nav.eph.contains_key(&id) {
                continue;
            }
            add(id, a.position(tow), false, -1.0, a.health == 0);
        }
        for (id, e) in &self.nav.glo_eph {
            let Some(leap) = self.leap_s(tow) else {
                continue;
            };
            let tod = (tow - leap as f64 + 10_800.0).rem_euclid(86_400.0);
            let (p, _) = e.integrate(tod, 60.0);
            add(*id, p, true, e.age(tod), e.healthy());
        }
        (sky, subs)
    }

    fn progress_lines(&self, si: &SearchInfo, now: f64) -> Vec<String> {
        let mut v = Vec::new();
        if self.fix.is_some() {
            return v;
        }
        if self.no_band {
            use crate::settings::Tune;
            let mut line = String::from("No band fits this tune and rate. Tunes:");
            for t in [Tune::L1, Tune::B1i, Tune::Glo] {
                let (_, rate) = t.centre_and_rate();
                let label = &<Tune as sdrkit_core::Choice>::CHOICES
                    [usize::from(sdrkit_core::Choice::index(t))]
                .label;
                let _ = write!(line, " {label} at {} Msps.", rate / 1e6);
            }
            v.push(line);
            return v;
        }
        if si.running {
            v.push(format!(
                "searching {} {}, {:.0}% of the round, window {:+.1} kHz ± {:.1} kHz, {:.0} ms",
                si.system.map_or("", System::name),
                si.prn,
                si.progress * 100.0,
                si.centre_hz / 1e3,
                si.half_hz / 1e3,
                si.ms
            ));
        }
        match si.stage {
            1 | 2 => v.push("searching wider for the radio's frequency error".into()),
            3..=5 => v.push("searching longer for weak signals".into()),
            _ => {}
        }
        if now > 60.0 && !self.channels.iter().any(|c| c.locked()) {
            v.push("nothing tracked after a minute: the antenna needs power (bias-tee or a powered LNA) and a clear view of the sky".into());
        }
        // Collecting the ephemeris.
        let mut best: Option<(String, usize, usize, Option<f64>)> = None;
        for c in self
            .channels
            .iter()
            .filter(|c| c.locked() && c.sat.system != System::Sbas)
        {
            if self.nav.eph.contains_key(&c.sat) || self.nav.glo_eph.contains_key(&c.sat) {
                continue;
            }
            let (p, of) = self.nav.parts(c.sat);
            if best.as_ref().is_none_or(|b| p > b.1) {
                best = Some((c.sat.name(), p, of, self.nav.seconds_left(c.sat, now)));
            }
        }
        if let Some((n, p, of, left)) = best {
            if p == 0 {
                v.push(
                    "waiting for the start of the message (up to 6 s after the bit sync)".into(),
                );
            } else {
                v.push(match left {
                    Some(l) => format!("collecting the ephemeris: {n} {p}/{of} subframes, about {l:.0} s left"),
                    None => format!("collecting the ephemeris: {n} {p}/{of} parts (each satellite sends it every 30 s)"),
                });
            }
        }
        let neph = self.nav.eph.len() + self.nav.glo_eph.len();
        if neph >= 4 {
            v.push("four or more ephemerides: the fix comes with the next measurement".into());
        }
        let lvl = self.cond.level_dbfs();
        if lvl < -45.0 {
            v.push("input level under -45 dBFS: raise the gain".into());
        }
        if self.cond.clip_percent() > 1.0 {
            v.push("clipping over 1 %: lower the gain".into());
        }
        v
    }

    /// The clock comparison: the computer's clock at the arrival of a piece.
    pub fn clock_sample(&mut self, unix_s: f64) {
        if !self.cfg.clock_compare || self.fs <= 0.0 {
            return;
        }
        let consumed =
            self.pieces as f64 * PIECE as f64 / self.fs + self.pending.len() as f64 / self.fs;
        let sample0 = unix_s - consumed;
        let window = (consumed / 30.0).floor();
        if window != self.clock_min.2 {
            self.clock_min = (self.clock_min.1, f64::INFINITY, window);
        }
        self.clock_min.1 = self.clock_min.1.min(sample0);
        let best = self.clock_min.0.min(self.clock_min.1);
        if let (Some(f), Some(utc), true) = (
            &self.fix,
            self.fix.as_ref().and_then(|f| f.utc),
            best.is_finite(),
        ) {
            let utc_of_sample0 = utc - f.signal_s;
            let off_ms = (best - utc_of_sample0) * 1e3;
            self.clock_offsets.push_back(off_ms);
            while self.clock_offsets.len() > 60 {
                self.clock_offsets.pop_front();
            }
        }
    }

    /// The good and bad navigation units of all channels, including those gone.
    pub fn frame_counts(&self) -> (u64, u64) {
        let g: u64 = self.channels.iter().map(|c| c.good).sum();
        let b: u64 = self.channels.iter().map(|c| c.bad).sum();
        (self.good_total + g, self.bad_total + b)
    }

    /// The report as of the last update.
    pub fn last_report(&self) -> &GnssReport {
        &self.report
    }

    /// The sample rate.
    pub fn rate(&self) -> f64 {
        self.fs
    }
    /// The centre.
    pub fn centre(&self) -> f64 {
        self.centre
    }
    /// Searched mask.
    pub fn masks(&self) -> (u8, u8, u8) {
        (self.cfg.systems, self.active_mask, self.searched_mask)
    }
    /// The signal time of the last reset.
    pub fn reset_time(&self) -> f64 {
        self.reset_marker
    }
}

fn push_cap(v: &mut VecDeque<f32>, x: f32, cap: usize) {
    v.push_back(x);
    while v.len() > cap {
        v.pop_front();
    }
}

fn new_slot(
    family: Family,
    band: usize,
    kind: SignalKind,
    carrier: f64,
    prns: Vec<u16>,
    cap: f64,
) -> Slot {
    Slot {
        family,
        band,
        kind,
        carrier,
        searcher: Searcher::new(kind),
        order: prns.clone(),
        prns,
        pos: 0,
        round: 0,
        found_round: false,
        prev_found: false,
        job: None,
        spare: None,
        confirming: None,
        budget: 0.0,
        cap,
        last_seg_end: 0,
        stage: 0,
        sleep_until: 0.0,
        sleep_len: 0.0,
        long_round: false,
        current: None,
        window: (0.0, 0.0, 16),
    }
}
