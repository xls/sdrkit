//! The server of a lightning network: it checks the datagrams of the stations (rate limit per address, MAC, replay guard),
//! keeps a table of stations, aligns the common-view clocks on the boundaries of the timing transmitter, matches the
//! detections of all stations and locates each strike.
//!
//! The server clock (`now_ns`, UTC) drives the replay guard, the matcher's wait and the ages; the caller supplies it.

use super::packet::{
    mac_key, Body, Detection, Freshness, Packet, PacketError, RateLimit, ReplayGuard,
    TimingReading,
};
use super::station::TimingTx;
use sdrkit_algo_locate::clock::{is_fine, TimeSource};
use sdrkit_algo_locate::commonview::{align, BoundaryReading};
use sdrkit_algo_locate::matcher::{self, Located, MatchCounters, Matcher, MatcherConfig};
use sdrkit_algo_locate::tdoa::Use;
use sdrkit_algo_locate::{LocateError, Propagation};
use sdrkit_util_geo::Position;
use std::collections::VecDeque;

/// The most stations in the table.
pub const MAX_STATIONS: usize = 64;
/// The most strikes kept, newest first.
pub const MAX_STRIKES: usize = 500;
/// The matcher runs every this many seconds of the server clock.
pub const MATCH_EVERY_S: f64 = 0.2;
/// The clock group of the common-view stations of a transmitter that no absolute station ties to UTC: this plus its id.
pub const OWN_SCALE_GROUP: u32 = 1000;

/// The settings of a [`Server`].
#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    /// The network key text; datagrams are signed with SHA-256 of it.
    pub network_key: String,
    /// The propagation model.
    pub propagation: Propagation,
    /// Fine stations that a fix needs, 3 to 64.
    pub min_stations: usize,
    /// The timing transmitter whose boundary readings align the common-view clocks.
    pub timing: Option<TimingTx>,
}

/// One station in the server's table.
#[derive(Clone, Debug, PartialEq)]
pub struct NetStation {
    /// Its id (FNV-1a of its name).
    pub id: u32,
    /// Its name, from its hello; empty until one arrives.
    pub name: String,
    /// Its position.
    pub position: Position,
    /// Its time source.
    pub source: Option<TimeSource>,
    /// The 1-sigma of its clock, s.
    pub clock_sigma_s: f64,
    /// The server clock when its last datagram arrived, ns.
    pub last_ns: i64,
    /// Its detections received.
    pub detections: u64,
    /// Its detections used in fixes.
    pub used: u64,
    /// Its newest boundary reading of the server's timing transmitter.
    pub reading: Option<TimingReading>,
    /// The correction of its common-view clock, s, once aligned.
    pub correction_s: Option<f64>,
    /// The 1-sigma of the correction, s.
    pub correction_sigma_s: f64,
    /// The corrected clock is UTC (up to whole symbol periods).
    pub tied_to_utc: bool,
}

impl NetStation {
    /// Whether its clock is absolute: GPS or GNSS.
    pub fn absolute(&self) -> bool {
        matches!(self.source, Some(TimeSource::Gps | TimeSource::Gnss))
    }

    /// Whether its clock can give fine detections now.
    pub fn fine(&self) -> bool {
        match self.source {
            Some(TimeSource::CommonView) => self.correction_s.is_some(),
            Some(s) => is_fine(s, self.clock_sigma_s),
            None => false,
        }
    }
}

/// One located strike.
#[derive(Clone, Debug, PartialEq)]
pub struct StrikeRow {
    /// Its number, counted from 1.
    pub seq: u64,
    /// The time of the strike, UTC ns (or the common-view scale when `utc` is false).
    pub utc_ns: i64,
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// The semi-major axis of the 95 % ellipse, km.
    pub major_km: f64,
    /// The semi-minor axis, km.
    pub minor_km: f64,
    /// The azimuth of the major axis, degrees.
    pub az_deg: f64,
    /// Stations used.
    pub stations: usize,
    /// Coarse stations that heard it.
    pub also_heard: usize,
    /// The rms of the residuals, µs.
    pub rms_us: f64,
    /// The chi-square of the fix.
    pub chi2: f64,
    /// Its degrees of freedom.
    pub dof: usize,
    /// No degree of freedom checks it.
    pub unchecked: bool,
    /// A second solution fits as well.
    pub ambiguous: bool,
    /// The time is UTC (false: the scale of a common-view clock whose epoch is unknown).
    pub utc: bool,
    /// The station indices used, in the table's order.
    pub used: Vec<usize>,
    /// The station indices that also heard it.
    pub heard: Vec<usize>,
}

/// What became of the datagrams.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServerCounters {
    /// Accepted.
    pub ok: u64,
    /// Refused by the MAC.
    pub mac: u64,
    /// Refused as replays.
    pub replays: u64,
    /// Refused as stale.
    pub stale: u64,
    /// Refused as malformed, short, long or foreign.
    pub malformed: u64,
    /// Refused by the rate limit.
    pub rate_limited: u64,
    /// Refused because the station table is full.
    pub full: u64,
}

/// Why a datagram was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The sender's address sent too many.
    RateLimited,
    /// The datagram does not decode.
    Packet(PacketError),
    /// Seen before.
    Replay,
    /// Too old or too far from the server clock.
    Stale,
    /// The station table is full.
    Full,
}

/// A detection waiting for the next matcher run.
#[derive(Clone, Copy, Debug)]
struct Incoming {
    station: usize,
    source: Option<TimeSource>,
    detection: Detection,
}

/// The server (module documentation).
pub struct Server {
    key: [u8; 32],
    timing: Option<TimingTx>,
    propagation: Propagation,
    limit: RateLimit,
    guard: ReplayGuard,
    stations: Vec<NetStation>,
    matcher: Matcher,
    incoming: Vec<Incoming>,
    strikes: VecDeque<StrikeRow>,
    fresh: Vec<StrikeRow>,
    counters: ServerCounters,
    seq: u64,
    last_run_ns: Option<i64>,
    located: Vec<Located>,
}

impl Server {
    /// A server with the settings `cfg`.
    ///
    /// # Errors
    ///
    /// [`LocateError`] for a propagation model or a station count out of range.
    pub fn new(cfg: ServerConfig) -> Result<Server, LocateError> {
        let matcher = Matcher::new(MatcherConfig {
            propagation: cfg.propagation,
            min_stations: cfg.min_stations,
            ..MatcherConfig::default()
        })?;
        Ok(Server {
            key: mac_key(&cfg.network_key),
            timing: cfg.timing,
            propagation: cfg.propagation,
            limit: RateLimit::default(),
            guard: ReplayGuard::default(),
            stations: Vec::new(),
            matcher,
            incoming: Vec::new(),
            strikes: VecDeque::new(),
            fresh: Vec::new(),
            counters: ServerCounters::default(),
            seq: 0,
            last_run_ns: None,
            located: Vec::new(),
        })
    }

    /// The station table, in the order the stations first sent.
    pub fn stations(&self) -> &[NetStation] {
        &self.stations
    }

    /// The located strikes, newest first.
    pub fn strikes(&self) -> &VecDeque<StrikeRow> {
        &self.strikes
    }

    /// Takes the strikes located since the last call, oldest first.
    pub fn take_new(&mut self) -> Vec<StrikeRow> {
        std::mem::take(&mut self.fresh)
    }

    /// What became of the datagrams.
    pub fn counters(&self) -> ServerCounters {
        self.counters
    }

    /// What became of the groups of detections.
    pub fn match_counters(&self) -> MatchCounters {
        self.matcher.counters()
    }

    /// Detections waiting to be matched.
    pub fn waiting(&self) -> usize {
        self.incoming.len() + self.matcher.pending()
    }

    /// Empties the strikes, the station table, the waiting detections and the counters. The replay guard is kept, so a
    /// datagram seen before stays refused.
    pub fn clear(&mut self) {
        self.stations.clear();
        self.matcher.clear();
        self.incoming.clear();
        self.strikes.clear();
        self.fresh.clear();
        self.counters = ServerCounters::default();
    }

    /// Takes a datagram from the address `from` at the server clock `now_ns`: the rate limit of the address, the decode with
    /// the MAC check, the replay guard, then the packet.
    ///
    /// # Errors
    ///
    /// [`Refusal`] with the first check that fails; it is counted.
    pub fn datagram(&mut self, bytes: &[u8], from: &str, now_ns: i64) -> Result<(), Refusal> {
        if !self.limit.allow(from, now_ns as f64 * 1e-9) {
            self.counters.rate_limited += 1;
            return Err(Refusal::RateLimited);
        }
        let p = match Packet::decode(bytes, &self.key) {
            Ok(p) => p,
            Err(e) => {
                match e {
                    PacketError::Mac => self.counters.mac += 1,
                    _ => self.counters.malformed += 1,
                }
                return Err(Refusal::Packet(e));
            }
        };
        match self.guard.check(p.station, p.seq, p.sent_ns, now_ns) {
            Freshness::Fresh => {}
            Freshness::Replay => {
                self.counters.replays += 1;
                return Err(Refusal::Replay);
            }
            Freshness::Stale => {
                self.counters.stale += 1;
                return Err(Refusal::Stale);
            }
            Freshness::Full => {
                self.counters.full += 1;
                return Err(Refusal::Full);
            }
        }
        self.accept(&p, now_ns)
    }

    /// Takes a packet that needs no check (the server's own station) at the server clock `now_ns`.
    ///
    /// # Errors
    ///
    /// [`Refusal::Full`] when the station is new and the table is full.
    pub fn accept(&mut self, p: &Packet, now_ns: i64) -> Result<(), Refusal> {
        let at = match self.stations.iter().position(|s| s.id == p.station) {
            Some(i) => i,
            None if self.stations.len() >= MAX_STATIONS => {
                self.counters.full += 1;
                return Err(Refusal::Full);
            }
            None => {
                self.stations.push(NetStation {
                    id: p.station,
                    name: String::new(),
                    position: Position::new(p.header.lat, p.header.lon),
                    source: None,
                    clock_sigma_s: 0.0,
                    last_ns: now_ns,
                    detections: 0,
                    used: 0,
                    reading: None,
                    correction_s: None,
                    correction_sigma_s: 0.0,
                    tied_to_utc: false,
                });
                self.stations.len() - 1
            }
        };
        self.counters.ok += 1;
        let tx = self.timing.as_ref().map(|t| t.id);
        let s = &mut self.stations[at];
        s.position = Position::new(p.header.lat, p.header.lon);
        s.last_ns = now_ns;
        match &p.body {
            Body::Hello(name) => name.clone_into(&mut s.name),
            Body::Timing(r) => {
                s.source = p.header.source;
                s.clock_sigma_s = f64::from(p.header.clock_sigma_ns) * 1e-9;
                if Some(r.tx_id) == tx {
                    s.reading = Some(*r);
                }
            }
            Body::Detections(d) => {
                s.source = p.header.source;
                s.clock_sigma_s = f64::from(p.header.clock_sigma_ns) * 1e-9;
                s.detections += d.len() as u64;
                self.incoming.extend(d.iter().map(|&detection| Incoming {
                    station: at,
                    source: p.header.source,
                    detection,
                }));
            }
        }
        Ok(())
    }

    /// At the server clock `now_ns`: when [`MATCH_EVERY_S`] has passed since the last run, aligns the common-view clocks,
    /// hands the waiting detections to the matcher and locates the groups that have waited long enough. Returns whether it
    /// ran.
    pub fn run(&mut self, now_ns: i64) -> bool {
        let every = (MATCH_EVERY_S * 1e9) as i64;
        if self
            .last_run_ns
            .is_some_and(|t| now_ns.saturating_sub(t) < every && now_ns >= t)
        {
            return false;
        }
        self.last_run_ns = Some(now_ns);
        self.align();
        for inc in std::mem::take(&mut self.incoming) {
            let d = self.detection(&inc);
            let _ = self.matcher.push(d);
        }
        let positions: Vec<Position> = self.stations.iter().map(|s| s.position).collect();
        let mut located = std::mem::take(&mut self.located);
        located.clear();
        self.matcher.run(now_ns, &positions, &mut located);
        for l in &located {
            self.record(l);
        }
        self.located = located;
        true
    }

    /// The common-view corrections from the newest boundary readings of the server's timing transmitter.
    fn align(&mut self) {
        let Some(tx) = &self.timing else {
            return;
        };
        let members: Vec<usize> = (0..self.stations.len())
            .filter(|&i| {
                let s = &self.stations[i];
                s.reading.is_some() && (s.absolute() || s.source == Some(TimeSource::CommonView))
            })
            .collect();
        let readings: Vec<BoundaryReading> = members
            .iter()
            .filter_map(|&i| {
                let s = &self.stations[i];
                s.reading.map(|r| BoundaryReading {
                    position: s.position,
                    boundary_ns: r.boundary_ns,
                    sigma_s: f64::from(r.sigma_ns) * 1e-9,
                    absolute: s.absolute(),
                })
            })
            .collect();
        let Ok(al) = align(&readings, &tx.position, tx.period_s(), &self.propagation) else {
            return;
        };
        for (k, &i) in members.iter().enumerate() {
            let s = &mut self.stations[i];
            if s.absolute() {
                s.correction_s = None;
                s.tied_to_utc = true;
            } else {
                s.correction_s = Some(al.corrections_s[k]);
                s.correction_sigma_s = al.sigmas_s[k];
                s.tied_to_utc = al.tied_to_utc;
            }
        }
    }

    /// The matcher's view of a detection: the common-view correction applied, its clock group, ambiguity and fineness.
    fn detection(&self, inc: &Incoming) -> matcher::Detection {
        let s = &self.stations[inc.station];
        let d = &inc.detection;
        let sigma = f64::from(d.sigma_ns.max(1)) * 1e-9;
        let base = matcher::Detection {
            station: inc.station,
            time_ns: d.utc_ns,
            sigma_s: sigma,
            group: matcher::UTC_GROUP,
            ambiguity_s: 0.0,
            width_s: (d.width_us * 1e-6).max(0.0),
            fine: false,
        };
        match inc.source {
            Some(TimeSource::CommonView) => match (s.correction_s, &self.timing) {
                (Some(c), Some(tx)) => {
                    let sigma = sigma.hypot(s.correction_sigma_s);
                    matcher::Detection {
                        time_ns: d.utc_ns.saturating_add((c * 1e9).round() as i64),
                        sigma_s: sigma,
                        group: if s.tied_to_utc {
                            matcher::UTC_GROUP
                        } else {
                            OWN_SCALE_GROUP + u32::from(tx.id)
                        },
                        ambiguity_s: tx.period_s(),
                        fine: is_fine(TimeSource::CommonView, sigma),
                        ..base
                    }
                }
                _ => matcher::Detection {
                    sigma_s: TimeSource::Ntp.typical_sigma_s(),
                    ..base
                },
            },
            Some(src) => matcher::Detection {
                fine: is_fine(src, sigma),
                ..base
            },
            None => base,
        }
    }

    /// Adds a located strike to the table and the new strikes.
    fn record(&mut self, l: &Located) {
        self.seq += 1;
        let used: Vec<usize> = l
            .detections
            .iter()
            .zip(&l.fix.uses)
            .filter(|(_, &u)| u == Use::Used)
            .map(|(d, _)| d.station)
            .collect();
        for &i in &used {
            if let Some(s) = self.stations.get_mut(i) {
                s.used += 1;
            }
        }
        let e = &l.fix.ellipse;
        let row = StrikeRow {
            seq: self.seq,
            utc_ns: l.time_ns,
            lat: l.fix.position.lat_deg,
            lon: l.fix.position.lon_deg,
            major_km: e.major_m * 1e-3,
            minor_km: e.minor_m * 1e-3,
            az_deg: e.azimuth_deg,
            stations: used.len(),
            also_heard: l.also_heard.len(),
            rms_us: l.fix.rms_s * 1e6,
            chi2: l.fix.chi2,
            dof: l.fix.dof,
            unchecked: l.fix.unchecked,
            ambiguous: l.fix.ambiguous,
            utc: l.utc,
            used,
            heard: l.also_heard.iter().map(|d| d.station).collect(),
        };
        self.strikes.push_front(row.clone());
        self.strikes.truncate(MAX_STRIKES);
        self.fresh.push(row);
    }
}
