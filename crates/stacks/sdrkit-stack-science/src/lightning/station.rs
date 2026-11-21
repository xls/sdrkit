//! One station of a lightning network: the sferic timer, the timer of a timing transmitter, the disciplined sample clocks,
//! and the detections, boundary readings and hellos that the station sends.
//!
//! The sample time is the absolute input index over the nominal rate. Clock marks come from the host (the computer clock), from
//! [`Station::mark`] (GPS or GNSS time that the host supplies) and from the symbol boundaries of the timing transmitter (common
//! view). Input is handled in pieces that end at the fixed indices where marks are made, so that a stream fed in pieces of any
//! size gives the same stamps.

use super::packet::{
    first_sequence, mac_key, station_id, Body, Detection, Packet, StationHeader, TimingReading,
    MAX_DETECTIONS,
};
use super::timer::{Pulse, SfericTimer};
use sdrkit_algo_locate::clock::{is_fine, ClockSet, Mark, MarkOutcome, Stamp, TimeSource};
use sdrkit_algo_locate::LocateError;
use sdrkit_algo_modem::msktiming::{MskReading, MskTimer};
use sdrkit_core::Complex32;
use sdrkit_util_geo::Position;

/// The computer clock is marked every this many seconds of samples (recordings) or at most this often (live samples), s.
pub const HOST_MARK_S: f64 = 0.5;
/// A hello goes out every this many seconds.
pub const HELLO_S: f64 = 10.0;
/// The fixed bias of a GPS mark: the receive chain's filters, s, plus two samples.
pub const GPS_BIAS_S: f64 = 100e-9;
/// The 1-sigma of a GPS mark, s.
pub const GPS_SIGMA_S: f64 = 20e-9;
/// The least 1-sigma of a GNSS mark, s.
pub const GNSS_MIN_SIGMA_S: f64 = 20e-9;
/// The time dilution of precision and the range residual that a GNSS mark assumes when none is given.
pub const GNSS_TDOP: f64 = 1.5;
/// The range residual of a GNSS mark when none is given, m.
pub const GNSS_RESIDUAL_M: f64 = 5.0;
/// The least 1-sigma of a computer-clock mark, s.
pub const HOST_MIN_SIGMA_S: f64 = 1e-6;

/// A timing transmitter: an MSK station whose symbol clock comes from a frequency standard.
#[derive(Clone, Debug, PartialEq)]
pub struct TimingTx {
    /// Its id, 1 to 255, the same at every station of a network.
    pub id: u8,
    /// Its label.
    pub label: String,
    /// Its carrier, Hz.
    pub freq_hz: f64,
    /// Its symbol rate, Bd.
    pub baud: f64,
    /// Where it is.
    pub position: Position,
}

impl TimingTx {
    /// The symbol period, s.
    pub fn period_s(&self) -> f64 {
        1.0 / self.baud
    }
}

/// Where the computer-clock marks of a station come from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostTime {
    /// None: every mark comes through [`Station::mark`].
    None,
    /// A recording: sample `n` lies at `start_utc_ns + n / rate` on the computer clock of `source`; a mark every
    /// [`HOST_MARK_S`] of samples, with the 1-sigma [`HOST_MIN_SIGMA_S`] and the fixed bias `bias_s`.
    File {
        /// UTC of sample 0, ns.
        start_utc_ns: i64,
        /// PTP, NTP or the system clock.
        source: TimeSource,
        /// The fixed bias of the clock, s.
        bias_s: f64,
    },
    /// Live samples: the host reads its clock when a group of samples arrives ([`Station::host_mark`]).
    Live {
        /// PTP, NTP or the system clock.
        source: TimeSource,
        /// The fixed bias of the clock, s.
        bias_s: f64,
    },
}

/// The settings of a [`Station`].
#[derive(Clone, Debug, PartialEq)]
pub struct StationConfig {
    /// The input rate, Hz.
    pub rate: f64,
    /// The start level of the sferic timer over the floor, dB.
    pub threshold_db: f64,
    /// The timing transmitter and its offset from the tuned centre, Hz, when one lies inside the capture.
    pub timing: Option<(TimingTx, f64)>,
    /// Where the computer-clock marks come from.
    pub host: HostTime,
}

/// Why a station cannot be made.
#[derive(Clone, Debug, PartialEq)]
pub enum StationError {
    /// The sferic timer refused a value.
    Timer(String),
    /// The timer of the timing transmitter refused a value.
    Timing(String),
}

impl std::error::Error for StationError {}

impl std::fmt::Display for StationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StationError::Timer(e) => write!(f, "sferic timer: {e}"),
            StationError::Timing(e) => write!(f, "timing transmitter: {e}"),
        }
    }
}

/// The common-view clock: the anchor of the transmitter's grid and the last boundary marked on it.
#[derive(Clone, Copy, Debug, Default)]
struct CommonView {
    /// UTC of the anchor boundary on the coarse clock, ns.
    anchor_ns: Option<i64>,
    /// The last boundary marked (input index) and its count of periods from the anchor.
    last: Option<(f64, i64)>,
    /// The restart count of the MSK timer at the anchor.
    msk_restarts: u64,
}

/// One station (module documentation).
pub struct Station {
    rate: f64,
    timer: SfericTimer,
    msk: Option<MskTimer>,
    timing: Option<TimingTx>,
    host: HostTime,
    host_every: u64,
    next_host_mark: u64,
    last_live_mark: Option<f64>,
    clocks: ClockSet,
    cv: CommonView,
    msk_origin: u64,
    msk_next: Option<u64>,
    next: u64,
    started: bool,
    pulses_buf: Vec<Pulse>,
    readings_buf: Vec<MskReading>,
    pending: Vec<(TimeSource, Detection)>,
    out_readings: Vec<TimingReading>,
    pulses: u64,
    unstamped: u64,
    last_peak_db: Option<f64>,
    identity: Option<(u32, String)>,
    position: Option<Position>,
    alt_m: i16,
    seq: Option<u32>,
    last_hello_s: Option<f64>,
}

impl Station {
    /// A station with the settings `cfg`.
    ///
    /// # Errors
    ///
    /// [`StationError`] when the sferic timer or the timer of the timing transmitter refuses a value.
    pub fn new(cfg: StationConfig) -> Result<Station, StationError> {
        let notch = cfg.timing.as_ref().map(|(_, off)| *off);
        let timer = SfericTimer::new(cfg.rate, cfg.threshold_db, notch)
            .map_err(|e| StationError::Timer(e.to_string()))?;
        let (msk, timing) = match cfg.timing {
            Some((tx, off)) => (
                Some(
                    MskTimer::new(cfg.rate, off, tx.baud)
                        .map_err(|e| StationError::Timing(e.to_string()))?,
                ),
                Some(tx),
            ),
            None => (None, None),
        };
        let host_every = ((HOST_MARK_S * cfg.rate).round() as u64).max(1);
        Ok(Station {
            rate: cfg.rate,
            timer,
            msk,
            timing,
            host: cfg.host,
            host_every,
            next_host_mark: 0,
            last_live_mark: None,
            clocks: ClockSet::new(),
            cv: CommonView::default(),
            msk_origin: 0,
            msk_next: None,
            next: 0,
            started: false,
            pulses_buf: Vec::new(),
            readings_buf: Vec::new(),
            pending: Vec::new(),
            out_readings: Vec::new(),
            pulses: 0,
            unstamped: 0,
            last_peak_db: None,
            identity: None,
            position: None,
            alt_m: 0,
            seq: None,
            last_hello_s: None,
        })
    }

    /// Names the station: its id is the FNV-1a hash of `name`. `None` keeps it unnamed (it sends nothing).
    pub fn set_name(&mut self, name: Option<&str>) {
        self.identity = name.map(|n| (station_id(n), n.to_owned()));
    }

    /// Places the station at `position`, `height_m` above the ellipsoid. `None` keeps it unplaced (it sends nothing).
    pub fn set_position(&mut self, position: Option<Position>, height_m: f64) {
        self.position = position;
        self.alt_m = height_m.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
    }

    /// The station id, when named.
    pub fn id(&self) -> Option<u32> {
        self.identity.as_ref().map(|i| i.0)
    }

    /// The position, when placed.
    pub fn position(&self) -> Option<Position> {
        self.position
    }

    /// Sets the start level of the sferic timer.
    pub fn set_threshold(&mut self, threshold_db: f64) {
        self.timer.set_threshold(threshold_db);
    }

    /// The input rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The timing transmitter in use.
    pub fn timing(&self) -> Option<&TimingTx> {
        self.timing.as_ref()
    }

    /// The sferic timer.
    pub fn timer(&self) -> &SfericTimer {
        &self.timer
    }

    /// The clocks, one per source.
    pub fn clocks(&self) -> &ClockSet {
        &self.clocks
    }

    /// The sample time of the next input sample, s.
    pub fn now_s(&self) -> f64 {
        self.next as f64 / self.rate
    }

    /// The best source usable now and its stamp of the newest sample.
    pub fn now_stamp(&self) -> Option<(TimeSource, Stamp)> {
        let now = self.now_s();
        self.clocks.stamp(now, now)
    }

    /// UTC of the newest sample by the best source, ns.
    pub fn now_ns(&self) -> Option<i64> {
        self.now_stamp().map(|(_, s)| s.utc_ns)
    }

    /// Whether a detection stamped now would be fine.
    pub fn fine(&self) -> bool {
        self.now_stamp().is_some_and(|(s, st)| is_fine(s, st.sigma_s))
    }

    /// Pulses timed since the last clear.
    pub fn pulses(&self) -> u64 {
        self.pulses
    }

    /// Pulses that no clock could stamp.
    pub fn unstamped(&self) -> u64 {
        self.unstamped
    }

    /// The peak of the newest pulse, dB over the floor.
    pub fn last_peak_db(&self) -> Option<f64> {
        self.last_peak_db
    }

    /// The result of the last group of the timing-transmitter timer, also while its first groups settle.
    pub fn last_timing(&self) -> Option<&MskReading> {
        self.msk.as_ref().and_then(MskTimer::last)
    }

    /// Detections stamped and not yet taken, with their sources.
    pub fn pending(&self) -> &[(TimeSource, Detection)] {
        &self.pending
    }

    /// Boundary readings not yet taken.
    pub fn readings(&self) -> &[TimingReading] {
        &self.out_readings
    }

    /// Adds a clock mark (GPS or GNSS time from the host, or any source in a test). See [`ClockSet::add`].
    ///
    /// # Errors
    ///
    /// [`LocateError`] for a mark out of range.
    pub fn mark(&mut self, mark: Mark) -> Result<MarkOutcome, LocateError> {
        self.clocks.add(mark)
    }

    /// A GPS mark: UTC `utc_ns` at input index `sample`, with the GPS 1-sigma and bias (the receive chain plus two samples).
    pub fn gps_mark(&self, sample: u64, utc_ns: i64) -> Mark {
        Mark {
            source: TimeSource::Gps,
            sample_s: sample as f64 / self.rate,
            utc_ns,
            sigma_s: GPS_SIGMA_S,
            bias_s: GPS_BIAS_S + 2.0 / self.rate,
        }
    }

    /// A GNSS mark: UTC `utc_ns` at input index `sample` with the 1-sigma `sigma_s` (0: from the assumed dilution and residual)
    /// and the stream-alignment error `bias_s`.
    pub fn gnss_mark(&self, sample: u64, utc_ns: i64, sigma_s: f64, bias_s: f64) -> Mark {
        let sigma = if sigma_s > 0.0 {
            sigma_s
        } else {
            GNSS_TDOP * GNSS_RESIDUAL_M / sdrkit_algo_locate::SPEED_OF_LIGHT_M_S
        };
        Mark {
            source: TimeSource::Gnss,
            sample_s: sample as f64 / self.rate,
            utc_ns,
            sigma_s: sigma.max(GNSS_MIN_SIGMA_S),
            bias_s: bias_s.max(0.0),
        }
    }

    /// The computer clock read `utc_ns` when the `len` samples that end at input index `last` arrived (live samples). At most
    /// one mark is kept every [`HOST_MARK_S`]; the 1-sigma is half the length of the group, at least 1 µs.
    pub fn host_mark(&mut self, last: u64, len: usize, utc_ns: i64) {
        let HostTime::Live { source, bias_s } = self.host else {
            return;
        };
        let s = last as f64 / self.rate;
        if self.last_live_mark.is_some_and(|t| s - t < HOST_MARK_S) {
            return;
        }
        self.last_live_mark = Some(s);
        let _ = self.clocks.add(Mark {
            source,
            sample_s: s,
            utc_ns,
            sigma_s: (0.5 * len as f64 / self.rate).max(HOST_MIN_SIGMA_S),
            bias_s,
        });
    }

    /// Learns the floor of the sferic timer again. The clocks and the timing-transmitter timer are kept.
    pub fn reset_detectors(&mut self) {
        self.timer.reset();
    }

    /// Drops the detections and readings waiting to be sent.
    pub fn discard(&mut self) {
        self.pending.clear();
        self.out_readings.clear();
    }

    /// Empties the counters. The clocks are kept.
    pub fn clear(&mut self) {
        self.pulses = 0;
        self.unstamped = 0;
        self.last_peak_db = None;
    }

    /// Takes `x`, whose first sample has the absolute index `first`.
    pub fn process(&mut self, x: &[Complex32], first: u64) {
        if !self.started {
            self.started = true;
            self.next_host_mark = first;
        }
        let end = first + x.len() as u64;
        let mut at = first;
        while at < end {
            let mut stop = end;
            if matches!(self.host, HostTime::File { .. }) {
                self.file_mark(at);
                stop = stop.min(self.next_host_mark);
            }
            if let Some(g) = self.group_end(at) {
                stop = stop.min(g);
            }
            let piece = &x[(at - first) as usize..(stop - first) as usize];
            self.piece(piece, at);
            at = stop;
        }
        self.next = end;
    }

    /// The input ended: times the pulse still being measured.
    pub fn flush(&mut self) {
        let mut pulses = std::mem::take(&mut self.pulses_buf);
        pulses.clear();
        self.timer.flush(&mut pulses);
        self.stamp(&pulses);
        self.pulses_buf = pulses;
    }

    /// Adds the computer-clock mark of a recording that falls at `at`.
    fn file_mark(&mut self, at: u64) {
        let HostTime::File {
            start_utc_ns,
            source,
            bias_s,
        } = self.host
        else {
            return;
        };
        if at < self.next_host_mark {
            return;
        }
        let n = at - at % self.host_every;
        let utc_ns = start_utc_ns.saturating_add((n as f64 / self.rate * 1e9).round() as i64);
        let _ = self.clocks.add(Mark {
            source,
            sample_s: n as f64 / self.rate,
            utc_ns,
            sigma_s: HOST_MIN_SIGMA_S,
            bias_s,
        });
        self.next_host_mark = n + self.host_every;
    }

    /// The input index after the end of the timing timer's group that `at` falls in.
    fn group_end(&mut self, at: u64) -> Option<u64> {
        let g = self.msk.as_ref()?.group_samples();
        if self.msk_next != Some(at) {
            // The timer starts its grid again at an index it did not expect.
            self.msk_origin = at;
        }
        Some(at + (g - (at - self.msk_origin) % g))
    }

    fn piece(&mut self, x: &[Complex32], at: u64) {
        let mut pulses = std::mem::take(&mut self.pulses_buf);
        pulses.clear();
        self.timer.process(x, at, &mut pulses);
        self.stamp(&pulses);
        self.pulses_buf = pulses;
        if let Some(m) = &mut self.msk {
            let mut readings = std::mem::take(&mut self.readings_buf);
            readings.clear();
            m.process(at, x, &mut readings);
            self.msk_next = Some(at + x.len() as u64);
            for r in &readings {
                self.common_view(r);
            }
            self.readings_buf = readings;
        }
    }

    /// Stamps the pulses with the best clock at the time each was known.
    fn stamp(&mut self, pulses: &[Pulse]) {
        for p in pulses {
            self.pulses += 1;
            self.last_peak_db = Some(p.peak_db);
            let now = p.given_at as f64 / self.rate;
            let Some((source, st)) = self.clocks.stamp(now, p.edge / self.rate) else {
                self.unstamped += 1;
                continue;
            };
            let sigma = st.sigma_s.hypot(p.sigma_s);
            self.pending.push((
                source,
                Detection {
                    utc_ns: st.utc_ns,
                    sigma_ns: sigma_ns(sigma),
                    peak_db: p.peak_db,
                    amplitude: p.amplitude as f32,
                    width_us: p.width_us,
                    rise_us: p.rise_us,
                    mean_hz: p.mean_hz,
                },
            ));
        }
    }

    /// The best stamp of `sample_s` by a source other than common view (the coarse clock that counts the periods).
    fn coarse_stamp(&self, now_s: f64, sample_s: f64) -> Option<Stamp> {
        TimeSource::ALL
            .into_iter()
            .filter(|&s| s != TimeSource::CommonView)
            .find(|&s| self.clocks.clock(s).usable(now_s))
            .and_then(|s| self.clocks.clock(s).stamp(sample_s))
    }

    /// Marks a locked boundary on the transmitter's grid and makes its boundary reading.
    fn common_view(&mut self, r: &MskReading) {
        let Some(tx) = &self.timing else {
            return;
        };
        if !r.locked {
            return;
        }
        let (id, period, baud) = (tx.id, tx.period_s(), tx.baud);
        let s = r.boundary / self.rate;
        let anchored = self.cv.anchor_ns.is_some() && r.restarts == self.cv.msk_restarts;
        let mut placed = false;
        if anchored {
            if let (Some(anchor), Some((lb, lk))) = (self.cv.anchor_ns, self.cv.last) {
                let k = lk + ((r.boundary - lb) / r.period).round() as i64;
                let mark = Mark {
                    source: TimeSource::CommonView,
                    sample_s: s,
                    utc_ns: anchor.saturating_add((k as f64 * period * 1e9).round() as i64),
                    sigma_s: r.sigma_s.max(1e-9),
                    bias_s: 0.0,
                };
                if let Ok(o) = self.clocks.add(mark) {
                    placed = o != MarkOutcome::Restarted;
                    self.cv.last = Some((r.boundary, k));
                }
            }
        }
        if !placed {
            // No anchor yet, a restart of the timer, or the clock restarted: the anchor is taken again on the coarse clock.
            let Some(coarse) = self.coarse_stamp(s, s) else {
                return;
            };
            self.clocks.reset_source(TimeSource::CommonView);
            self.cv = CommonView {
                anchor_ns: Some(coarse.utc_ns),
                last: Some((r.boundary, 0)),
                msk_restarts: r.restarts,
            };
            let _ = self.clocks.add(Mark {
                source: TimeSource::CommonView,
                sample_s: s,
                utc_ns: coarse.utc_ns,
                sigma_s: r.sigma_s.max(1e-9),
                bias_s: 0.0,
            });
        }
        if let Some((source, st)) = self.clocks.stamp(s, s) {
            let sigma = if source == TimeSource::CommonView {
                r.sigma_s
            } else {
                r.sigma_s.hypot(st.sigma_s)
            };
            self.out_readings.push(TimingReading {
                tx_id: id,
                boundary_ns: st.utc_ns,
                sigma_ns: sigma_ns(sigma),
                snr_db: r.snr_db,
                baud: baud as f32,
            });
        }
    }

    /// What the station says about itself now: its position, the best source and its 1-sigma.
    pub fn header(&self) -> StationHeader {
        let p = self.position.unwrap_or(Position::new(0.0, 0.0));
        let best = self.now_stamp();
        StationHeader {
            lat: p.lat_deg,
            lon: p.lon_deg,
            alt_m: self.alt_m,
            source: best.map(|b| b.0),
            position_by_user: true,
            common_view: best.is_some_and(|b| b.0 == TimeSource::CommonView),
            clock_sigma_ns: best.map_or(u32::MAX, |b| sigma_ns(b.1.sigma_s)),
        }
    }

    /// Takes the detections, readings and the hello due as packets sent at `now_ns`: detections in batches of at most
    /// [`MAX_DETECTIONS`] of one source each, one packet per reading, and a hello every [`HELLO_S`]. A station without a name
    /// or a position sends nothing, and what it holds is dropped.
    pub fn packets(&mut self, now_ns: i64, out: &mut Vec<Packet>) {
        let (Some((id, name)), Some(_)) = (self.identity.clone(), self.position) else {
            self.pending.clear();
            self.out_readings.clear();
            return;
        };
        let header = self.header();
        let mut seq = self.seq.unwrap_or_else(|| first_sequence(now_ns));
        let mut packet = |body: Body, source| {
            let p = Packet {
                station: id,
                seq,
                sent_ns: now_ns,
                header: StationHeader { source, ..header },
                body,
            };
            seq = seq.wrapping_add(1);
            p
        };
        let now_s = self.now_s();
        if self.last_hello_s.is_none_or(|t| now_s - t >= HELLO_S) {
            self.last_hello_s = Some(now_s);
            out.push(packet(Body::Hello(name), header.source));
        }
        let pending = std::mem::take(&mut self.pending);
        let mut sources: Vec<TimeSource> = Vec::new();
        for (s, _) in &pending {
            if !sources.contains(s) {
                sources.push(*s);
            }
        }
        for s in sources {
            let batch: Vec<Detection> = pending
                .iter()
                .filter(|(src, _)| *src == s)
                .map(|(_, d)| *d)
                .collect();
            for chunk in batch.chunks(MAX_DETECTIONS) {
                out.push(packet(Body::Detections(chunk.to_vec()), Some(s)));
            }
        }
        for r in std::mem::take(&mut self.out_readings) {
            out.push(packet(Body::Timing(r), header.source));
        }
        self.seq = Some(seq);
    }

    /// As [`Station::packets`], encoded and signed with the network key text `key`.
    pub fn datagrams(&mut self, now_ns: i64, key: &str, out: &mut Vec<Vec<u8>>) {
        let mut packets = Vec::new();
        self.packets(now_ns, &mut packets);
        let k = mac_key(key);
        out.extend(packets.iter().map(|p| p.encode(&k)));
    }
}

/// A 1-sigma in whole nanoseconds, saturating.
fn sigma_ns(sigma_s: f64) -> u32 {
    (sigma_s * 1e9).round().clamp(1.0, f64::from(u32::MAX)) as u32
}
