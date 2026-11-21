//! The channel scan: for each channel of a list, the occupancy, the lock attempt, the width and the service list. The scan
//! requests each tuning from the engine and reads the results of the receiver.

use crate::params::{Bandwidth, CodeRate, Constellation, Guard, Mode};
use crate::results::Decoding;
use crate::typed::DvbTerrestrialFields;
use crate::typed::{DvbtFields as F, DvbtMultiplexFields as M};
use sdrkit_core::{LockState, Results};
use sdrkit_proto_mpegts::publish::{NAME, SERVICES, SERVICE_ID, SERVICE_KIND};
use sdrkit_proto_mpegts::si::ServiceKind;

sdrkit_core::choice_enum! {
    /// The reason a scanned channel has no multiplex.
    pub enum ScanNote {
        /// Nothing above the floor.
        Empty = ("empty", "Empty"),
        /// A signal, but neither DVB-T2 nor DVB-T locked.
        NoLock = ("no_lock", "Signal present, no DVB-T2 or DVB-T"),
        /// No signal of the standard.
        NoSignal = ("no_signal", "No signal"),
    }
}

/// One service of a scanned multiplex.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanService {
    /// The service id.
    pub id: u16,
    /// The name, empty when not known yet.
    pub name: String,
    /// What the service carries.
    pub kind: Option<ServiceKind>,
}

/// A request of the scan to the engine.
#[derive(Clone, Debug, PartialEq)]
pub enum ScanCommand {
    /// Tune here, with this bandwidth (None: Auto).
    Tune {
        /// Frequency.
        hz: f64,
        /// Bandwidth.
        bandwidth: Option<Bandwidth>,
    },
    /// The scan is over (return to this frequency).
    Done {
        /// Where to go back to.
        hz: f64,
    },
}

/// One scanned channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanResult {
    /// Frequency.
    pub frequency_hz: f64,
    /// The channel label ("28", "9A"). Empty for a start, stop and step scan.
    pub channel: String,
    /// Bandwidth, MHz.
    pub bandwidth_mhz: u32,
    /// Level, dBFS.
    pub level_dbfs: f32,
    /// In-band against out-of-band, dB.
    pub occupancy_db: f32,
    /// The signal of the standard was seen (a cyclic prefix).
    pub seen: bool,
    /// It locked.
    pub locked: bool,
    /// The standard that decoded.
    pub standard: Decoding,
    /// The FFT mode.
    pub mode: Option<Mode>,
    /// The guard interval.
    pub guard: Option<Guard>,
    /// The constellation.
    pub constellation: Option<Constellation>,
    /// The HP code rate.
    pub code_rate: Option<CodeRate>,
    /// SNR, dB.
    pub snr_db: f32,
    /// Cell id.
    pub cell_id: u16,
    /// Network id.
    pub network_id: u16,
    /// Network name.
    pub network_name: String,
    /// The services in list order.
    pub services: Vec<ScanService>,
    /// Multiplex bit rate, bit/s.
    pub bitrate: f64,
    /// Fraction of good RS blocks.
    pub rs_good: f32,
    /// The reason the channel has no multiplex.
    pub note: Option<ScanNote>,
}

impl Default for ScanResult {
    fn default() -> Self {
        ScanResult {
            frequency_hz: 0.0,
            channel: String::new(),
            bandwidth_mhz: 0,
            level_dbfs: 0.0,
            occupancy_db: 0.0,
            seen: false,
            locked: false,
            standard: Decoding::Dvbt,
            mode: None,
            guard: None,
            constellation: None,
            code_rate: None,
            snr_db: 0.0,
            cell_id: 0,
            network_id: 0,
            network_name: String::new(),
            services: Vec::new(),
            bitrate: 0.0,
            rs_good: 0.0,
            note: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Settle,
    Lock,
    Services,
}

/// A scan in progress.
pub struct ChannelScan {
    list: Vec<(f64, Option<Bandwidth>)>,
    labels: Vec<String>,
    at: usize,
    phase: Phase,
    t0: f64,
    names_at: Option<f64>,
    /// Results so far.
    pub results: Vec<ScanResult>,
    return_hz: f64,
    stopped: bool,
    current: ScanResult,
}

/// How long to wait for a lock.
pub const LOCK_S: f64 = 5.0;
/// How long to read the service list.
pub const SERVICES_S: f64 = 9.0;
/// Signal time to settle after a retune before the occupancy is measured.
pub const SETTLE_S: f64 = 0.2;
/// The longest wait for an occupancy measurement. Without one the channel is tried rather than skipped.
pub const SETTLE_MAX_S: f64 = 1.0;
/// The in-band level above the out-of-band floor that makes a channel occupied, dB.
pub const OCCUPIED_DB: f32 = 5.0;
/// With TPS decoded and less than this left of the lock wait, the scan stops waiting for packets.
pub const LOCK_ENOUGH_S: f64 = 0.8;
/// After the first service names appear, the service list is read for at most this long.
pub const NAMES_S: f64 = 3.0;

impl ChannelScan {
    /// A scan of labelled channels: (label, centre frequency, width), in the order given.
    pub fn channels(channels: &[(&str, f64, Option<Bandwidth>)], return_hz: f64) -> Self {
        let mut scan = Self::list(channels.iter().map(|c| (c.1, c.2)).collect(), return_hz);
        scan.labels = channels.iter().map(|c| c.0.to_owned()).collect();
        scan
    }
    /// A scan from `start` to `stop` in `step` (Auto bandwidth).
    pub fn range(start_hz: f64, stop_hz: f64, step_hz: f64, return_hz: f64) -> Self {
        let mut v = Vec::new();
        let mut f = start_hz;
        while f <= stop_hz + 1.0 {
            v.push((f, None));
            f += step_hz;
        }
        Self::list(v, return_hz)
    }
    /// A scan of any list.
    pub fn list(list: Vec<(f64, Option<Bandwidth>)>, return_hz: f64) -> Self {
        ChannelScan {
            labels: vec![String::new(); list.len()],
            list,
            at: 0,
            phase: Phase::Settle,
            t0: 0.0,
            names_at: None,
            results: Vec::new(),
            return_hz,
            stopped: false,
            current: ScanResult::default(),
        }
    }
    /// The first command.
    pub fn start(&mut self, now: f64) -> ScanCommand {
        self.at = 0;
        self.begin(now)
    }
    /// Stops the scan (the results so far stay).
    pub fn stop(&mut self) -> ScanCommand {
        self.stopped = true;
        ScanCommand::Done { hz: self.return_hz }
    }
    /// Whether the scan is over.
    pub fn done(&self) -> bool {
        self.stopped || self.at >= self.list.len()
    }
    /// Channels done and channels in all.
    pub fn progress(&self) -> (usize, usize) {
        (self.at.min(self.list.len()), self.list.len())
    }
    /// The frequency being scanned now.
    pub fn current_hz(&self) -> Option<f64> {
        (!self.done()).then(|| self.list[self.at].0)
    }
    /// The multiplexes found (channels that locked).
    pub fn multiplexes(&self) -> impl Iterator<Item = &ScanResult> {
        self.results.iter().filter(|r| r.locked)
    }
    /// Every service found, with the channel it is on: (frequency, channel label, service).
    pub fn services(&self) -> Vec<(f64, &str, &ScanService)> {
        self.multiplexes()
            .flat_map(|r| {
                r.services
                    .iter()
                    .map(move |s| (r.frequency_hz, r.channel.as_str(), s))
            })
            .collect()
    }
    fn begin(&mut self, now: f64) -> ScanCommand {
        if self.done() {
            return ScanCommand::Done { hz: self.return_hz };
        }
        let (hz, bw) = self.list[self.at];
        self.phase = Phase::Settle;
        self.t0 = now;
        self.names_at = None;
        self.current = ScanResult {
            frequency_hz: hz,
            channel: self.labels[self.at].clone(),
            bandwidth_mhz: bw.map_or(0, |b| b.mhz()),
            ..Default::default()
        };
        ScanCommand::Tune { hz, bandwidth: bw }
    }
    fn next(&mut self, now: f64) -> ScanCommand {
        let r = std::mem::take(&mut self.current);
        self.results.push(r);
        self.at += 1;
        self.begin(now)
    }

    /// The receiver's results at signal time `now`. Returns a command when the scan moves on.
    pub fn on_report(&mut self, now: f64, r: &Results) -> Option<ScanCommand> {
        if self.done() {
            return None;
        }
        let f = r.fields();
        // DVB-T: TPS decoded. DVB-T2 (through the DVB-T2 / DVB-T stack): its L1 signalling decoded. Either lock state says locked.
        let locked = r.status().lock_state == LockState::Locked;
        let elapsed = now - self.t0;
        let c = &mut self.current;
        c.level_dbfs = f.at(&F::LEVEL_DBFS).unwrap_or(-120.0) as f32;
        // Results from before the tune can still show for a moment: detection counts after the settle time.
        let settled = self.phase != Phase::Settle || elapsed >= SETTLE_S;
        if settled && f.at(&F::DETECT_LEVEL).unwrap_or(0) >= 1 {
            c.seen = true;
        }
        if let Some(d) = f.at(&DvbTerrestrialFields::STANDARD) {
            c.standard = d;
        }
        match self.phase {
            Phase::Settle => {
                let occupancy = f.at(&F::OCCUPANCY_DB);
                if elapsed >= SETTLE_S && (occupancy.is_some() || elapsed >= SETTLE_MAX_S) {
                    c.occupancy_db = occupancy.unwrap_or(0.0) as f32;
                    // Empty channels are skipped at once. Without a measurement the channel is tried.
                    if occupancy.is_some() && c.occupancy_db < OCCUPIED_DB && !c.seen {
                        c.note = Some(ScanNote::Empty);
                        return Some(self.next(now));
                    }
                    self.phase = Phase::Lock;
                    self.t0 = now;
                }
            }
            Phase::Lock => {
                let data = f.at(&F::DATA_VALID) == Some(true) && f.at(&F::FEC_SYNC) == Some(true);
                if locked {
                    c.locked = true;
                }
                let left = LOCK_S - elapsed;
                if (locked && data) || (locked && left < LOCK_ENOUGH_S) {
                    self.phase = Phase::Services;
                    self.t0 = now;
                } else if left <= 0.0 {
                    c.note = Some(if c.seen {
                        ScanNote::NoLock
                    } else {
                        ScanNote::NoSignal
                    });
                    return Some(self.next(now));
                }
            }
            Phase::Services => {
                c.mode = f.at(&F::FFT);
                c.guard = f.at(&F::GUARD);
                c.constellation = f.at(&F::CONSTELLATION);
                c.code_rate = f.at(&F::CODE_RATE_HP);
                c.snr_db = f.at(&F::SNR_DB).unwrap_or(0.0) as f32;
                c.cell_id = f.at(&F::CELL_ID).unwrap_or(0) as u16;
                c.network_name = f.at(&M::NETWORK_NAME).unwrap_or("").to_string();
                c.bitrate = f.at(&F::NET_BITRATE_BPS).unwrap_or(0.0);
                c.bandwidth_mhz = (f.at(&F::CHANNEL_WIDTH).unwrap_or(0.0_f64) / 1e6).round() as u32;
                let blocks = f.at(&F::RS_BLOCKS).unwrap_or(0);
                c.rs_good = if blocks > 0 {
                    (1.0 - f.at(&F::RS_FAILED).unwrap_or(0) as f64 / blocks as f64) as f32
                } else {
                    0.0
                };
                // The table's rows are written in list order (by LCN, then service id). Newest first is that order reversed.
                c.services.clear();
                if let Some(t) = r.decl().table_id(SERVICES.key).and_then(|id| r.table(id)) {
                    for (_, row) in t.rows_newest_first() {
                        c.services.push(ScanService {
                            id: row.get(&SERVICE_ID).unwrap_or(0) as u16,
                            name: row.get(&NAME).unwrap_or("").to_string(),
                            kind: row.get(&SERVICE_KIND),
                        });
                    }
                }
                c.services.reverse();
                let all_named = !c.services.is_empty()
                    && c.services
                        .iter()
                        .all(|s| !s.name.is_empty() && s.name != format!("Service {}", s.id));
                if !c.services.is_empty() && self.names_at.is_none() {
                    self.names_at = Some(now);
                }
                c.network_id = f.at(&M::NETWORK_ID).unwrap_or(0) as u16;
                let named_long = self.names_at.is_some_and(|t| now - t >= NAMES_S);
                if (all_named && !c.network_name.is_empty()) || named_long || elapsed >= SERVICES_S
                {
                    return Some(self.next(now));
                }
            }
        }
        None
    }
}
