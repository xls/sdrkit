//! The receiver of a capture of the BGAN forward band: the family carrier detector finds the carriers, each carrier of a BGAN
//! width gets a demodulator for the symbol rates of its width class, and a wide carrier that does not lock is tested for the
//! spectral lines of a GMR-2 class carrier. The broadcast signalling of every carrier fills the carrier and beam tables, the
//! summary and the log.

use super::demod::{Demod, DemodOptions, DemodState, FrameResult};
use super::pdu::{self, Avp, Sdu};
use super::tables;
use crate::modes::inmarsat::{Detector, Limits};
use sdrkit_algo_dsp::fft::Fft;
use sdrkit_algo_dsp::spectrum::percentile;
use sdrkit_core::Complex32;
use std::collections::{BTreeMap, BTreeSet};

/// Seconds of input kept to start a new demodulator on.
pub const HISTORY_S: f64 = 0.6;
/// The symbol rate of a GMR-2 class carrier, symbols per second.
pub const GMR2_RATE: f64 = 135_416.67;
/// Lines in the squared spectrum must stand this far above its median.
const GMR2_LINE: f32 = 12.0;
/// The most carriers demodulated at once.
pub const MAX_DEMODS: usize = 16;
/// Regions narrower than this are not carriers, Hz.
pub const MIN_WIDTH_HZ: f64 = 1.5e3;
/// Lines kept in the log.
pub const LOG_LINES: usize = 200;

/// The symbol rates a carrier of width `w` Hz may have, most likely first; empty when the width is not one of BGAN.
pub fn rates_for_width(w: f64) -> Vec<f64> {
    if (6e3..16e3).contains(&w) {
        vec![8_400.0]
    } else if (24e3..62e3).contains(&w) {
        vec![33_600.0]
    } else if (62e3..130e3).contains(&w) {
        vec![84_000.0]
    } else if (130e3..225e3).contains(&w) {
        vec![168_000.0, 151_200.0]
    } else {
        Vec::new()
    }
}

/// What a carrier is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// Not identified yet.
    #[default]
    Unknown,
    /// A BGAN forward bearer.
    Bgan,
    /// A carrier with the lines of filtered OQPSK at 135.4 ksym/s (the IsatPhone type).
    Gmr2,
    /// Something else.
    Other,
}

impl Kind {
    /// The text of the carrier table.
    pub fn text(self) -> &'static str {
        match self {
            Kind::Unknown => "not identified",
            Kind::Bgan => "BGAN forward bearer",
            Kind::Gmr2 => "GMR-2 class (IsatPhone type)",
            Kind::Other => "other",
        }
    }
}

/// A satellite named by a SatelliteLocation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SatPos {
    /// Satellite id.
    pub id: u8,
    /// Degrees east.
    pub lon_deg: f64,
}

/// One carrier of the band.
#[derive(Default)]
pub struct Carrier {
    /// Row id.
    pub id: u64,
    /// The id of the detector's carrier.
    det_id: Option<u64>,
    /// Hz from the centre of the capture.
    pub offset_hz: f64,
    /// Absolute frequency when the dial is known, Hz.
    pub freq_hz: f64,
    /// Width, Hz.
    pub width_hz: f64,
    /// Over the floor, dB.
    pub snr_db: f64,
    /// What it is.
    pub kind: Kind,
    /// The demodulator.
    demod: Option<Demod>,
    rates: Vec<f64>,
    rate_at: usize,
    searches_at_rate: u32,
    gmr_tested: bool,
    /// The state of its demodulator.
    pub state: DemodState,
    /// Bearer name.
    pub bearer: String,
    /// Subtype of block 0.
    pub subtype: String,
    /// Symbols per second.
    pub symbol_rate: f64,
    /// Frequency left after the channel filter, Hz.
    pub cfo_hz: f64,
    /// C/N0, dB-Hz.
    pub cn0_dbhz: f64,
    /// Frames in sync.
    pub frames: u64,
    /// Blocks decoded.
    pub blocks_ok: u64,
    /// Blocks that failed.
    pub blocks_bad: u64,
    /// Blocks with PDUs.
    pub blocks_used: u64,
    /// Connection PDUs counted (never read).
    pub connection_pdus: u64,
    /// PDUs addressed to a terminal counted (never read).
    pub addressed_pdus: u64,
    /// Frame structure without decodable tables.
    pub structure_only: bool,
    /// From the bulletin board.
    pub spot: Option<u8>,
    /// rnc-id.
    pub rnc: Option<u8>,
    /// bct-id.
    pub bct_id: Option<u8>,
    /// f-bearer.
    pub f_bearer: Option<u8>,
    /// Beam type of BeamInfo.
    pub beam_type: Option<u8>,
    /// MCC-MNC.
    pub network: String,
    /// The carrying satellite.
    pub satellite: Option<SatPos>,
    /// Seconds since the start, last time heard.
    pub last_seen: f64,
    /// The last bulletin line logged (without the frame number).
    last_line: String,
    /// Pieces of the last frame's signalling.
    pub frame_no: Option<u16>,
}

impl Carrier {
    /// The share of FEC blocks that carried PDUs.
    pub fn load(&self) -> f64 {
        let n = self.blocks_ok + self.blocks_bad;
        if n == 0 {
            0.0
        } else {
            self.blocks_used as f64 / n as f64
        }
    }
}

/// A primary bearer announced for a spot beam.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Beam {
    /// The beam type when a carrier of this spot told it.
    pub beam_type: Option<u8>,
    /// The forward frequency of the primary bearer, Hz.
    pub primary_hz: Option<f64>,
    /// The FBearerType of the primary bearer.
    pub bearer_type: Option<u8>,
}

/// The receiver of the band.
pub struct BganRx {
    rate: f64,
    /// The dial (the frequency of the centre of the samples), Hz; 0 when unknown.
    pub tuned_hz: f64,
    opts: DemodOptions,
    det: Detector,
    history: Vec<Complex32>,
    /// Samples since the start.
    pub samples: u64,
    /// The carriers.
    pub carriers: Vec<Carrier>,
    next_id: u64,
    /// Beams by spot beam id.
    pub beams: BTreeMap<u8, Beam>,
    /// MCC-MNC heard.
    pub networks: BTreeSet<String>,
    /// Satellites heard.
    pub satellites: Vec<SatPos>,
    /// The log, newest last.
    pub log: Vec<(f64, String)>,
    /// New log lines since the last take.
    fresh: Vec<String>,
    /// The noise floor of the last search, dB of the median bin.
    pub floor_db: f64,
}

impl BganRx {
    /// A receiver at `rate` with `opts`.
    pub fn new(rate: f64, tuned_hz: f64, opts: DemodOptions) -> Self {
        let mut det = Detector::default();
        det.limits = Limits {
            eval_s: 0.2,
            max_width_hz: 260e3,
            gap_bins: 12,
            drop_s: 30.0,
            dc_hz: 0.0,
        };
        det.start(rate);
        BganRx {
            rate,
            tuned_hz,
            opts,
            det,
            history: Vec::new(),
            samples: 0,
            carriers: Vec::new(),
            next_id: 1,
            beams: BTreeMap::new(),
            networks: BTreeSet::new(),
            satellites: Vec::new(),
            log: Vec::new(),
            fresh: Vec::new(),
            floor_db: 0.0,
        }
    }

    /// The options of new demodulators.
    pub fn set_options(&mut self, opts: DemodOptions) {
        self.opts = opts;
        for c in &mut self.carriers {
            c.demod = None;
            c.rate_at = 0;
            c.searches_at_rate = 0;
        }
    }

    /// A new dial: the detector starts again; carrier rows stay with their last state.
    pub fn retune(&mut self, tuned_hz: f64) {
        self.tuned_hz = tuned_hz;
        self.det.start(self.rate);
        self.history.clear();
        for c in &mut self.carriers {
            c.demod = None;
            c.det_id = None;
            c.state = DemodState::Searching;
        }
    }

    /// Seconds since the start.
    pub fn now(&self) -> f64 {
        self.samples as f64 / self.rate
    }

    /// Takes the log lines added since the last call.
    pub fn take_fresh(&mut self) -> Vec<String> {
        std::mem::take(&mut self.fresh)
    }

    /// The count of carriers of `kind`.
    pub fn count(&self, kind: Kind) -> usize {
        self.carriers.iter().filter(|c| c.kind == kind).count()
    }

    /// Takes input samples.
    pub fn process(&mut self, x: &[Complex32]) {
        self.det.feed(x);
        self.samples += x.len() as u64;
        // New demodulators start on the samples before `x`; every demodulator then takes `x`.
        self.follow_detector();
        let keep = (HISTORY_S * self.rate) as usize;
        self.history.extend_from_slice(x);
        // Trimmed once it holds twice what is kept, so the samples move seldom.
        if self.history.len() > 2 * keep {
            let d = self.history.len() - keep;
            self.history.drain(..d);
        }
        let now = self.now();
        for i in 0..self.carriers.len() {
            let frames = match self.carriers[i].demod.as_mut() {
                Some(d) => d.process(x),
                None => continue,
            };
            for f in frames {
                self.frame(i, &f, now);
            }
            self.after_demod(i);
        }
    }

    /// New carriers of the detector get rows and demodulators; the rows of known ones follow it.
    fn follow_detector(&mut self) {
        let det = self.det.carriers.clone();
        for d in det.iter().filter(|d| d.width_hz >= MIN_WIDTH_HZ) {
            let abs = self.tuned_hz + d.offset_hz;
            let idx = self.carriers.iter().position(|c| {
                c.det_id == Some(d.id)
                    || (c.det_id.is_none() && (c.freq_hz - abs).abs() < (d.width_hz / 4.0).max(2e3))
            });
            let i = match idx {
                Some(i) => i,
                None => {
                    self.carriers.push(Carrier {
                        id: self.next_id,
                        rates: rates_for_width(d.width_hz),
                        ..Default::default()
                    });
                    self.next_id += 1;
                    self.carriers.len() - 1
                }
            };
            let demods = self.carriers.iter().filter(|c| c.demod.is_some()).count();
            let c = &mut self.carriers[i];
            c.det_id = Some(d.id);
            // A locked carrier keeps the frequency its demodulator measures.
            if !c.demod.as_ref().is_some_and(Demod::locked) {
                c.offset_hz = d.offset_hz;
                c.freq_hz = abs;
            }
            c.width_hz = d.width_hz;
            c.snr_db = d.snr_db;
            c.last_seen = c.last_seen.max(d.last_seen);
            if c.rates.is_empty() {
                c.rates = rates_for_width(d.width_hz);
                if c.rates.is_empty() && c.kind == Kind::Unknown {
                    c.kind = Kind::Other;
                }
            }
            if c.demod.is_none() && !c.rates.is_empty() && demods < MAX_DEMODS && c.kind != Kind::Gmr2 {
                let rate = c.rates[c.rate_at % c.rates.len()];
                c.symbol_rate = rate;
                if let Some(mut dm) = Demod::new(self.rate, d.offset_hz, rate, self.opts) {
                    // Start on the samples kept, so the first frames are not lost while the carrier was confirmed.
                    let keep = (HISTORY_S * self.rate) as usize;
                    let _ = dm.process(&self.history[self.history.len().saturating_sub(keep)..]);
                    c.demod = Some(dm);
                }
            }
        }
    }

    /// Rate changes and the GMR-2 test of a carrier whose demodulator does not lock.
    fn after_demod(&mut self, i: usize) {
        let rate = self.rate;
        let tuned = self.tuned_hz;
        let c = &mut self.carriers[i];
        let Some(d) = c.demod.as_ref() else { return };
        c.state = d.state;
        c.cfo_hz = d.cfo_hz;
        if d.locked() {
            c.offset_hz = d.offset_hz + d.cfo_hz;
            c.freq_hz = tuned + c.offset_hz;
            c.kind = Kind::Bgan;
            c.searches_at_rate = 0;
            return;
        }
        if d.failed_searches == 0 || u64::from(d.failed_searches) == u64::from(c.searches_at_rate) {
            return;
        }
        c.searches_at_rate = d.failed_searches;
        if d.failed_searches < 2 {
            return;
        }
        if c.rates.len() > 1 && c.rate_at + 1 < c.rates.len() * 2 {
            c.rate_at += 1;
            let r = c.rates[c.rate_at % c.rates.len()];
            if let Some(mut dm) = Demod::new(rate, c.offset_hz, r, self.opts) {
                let keep = (HISTORY_S * rate) as usize;
                let _ = dm.process(&self.history[self.history.len().saturating_sub(keep)..]);
                c.symbol_rate = r;
                c.searches_at_rate = 0;
                c.demod = Some(dm);
            }
            return;
        }
        if c.kind == Kind::Unknown && !c.gmr_tested {
            c.gmr_tested = true;
            let wide = (130e3..225e3).contains(&c.width_hz);
            let gmr = wide && gmr2_lines(d.recent(), SPS_RATE * c.symbol_rate);
            c.kind = if gmr { Kind::Gmr2 } else { Kind::Other };
            if gmr {
                c.demod = None;
                c.bearer.clear();
            }
        }
    }

    /// What a frame of carrier `i` gave.
    fn frame(&mut self, i: usize, f: &FrameResult, now: f64) {
        let c = &mut self.carriers[i];
        c.kind = Kind::Bgan;
        c.frames += 1;
        c.blocks_ok += u64::from(f.blocks_ok);
        c.blocks_bad += u64::from(f.blocks_bad);
        c.blocks_used += u64::from(f.blocks_used);
        c.bearer = f.bearer.to_string();
        c.subtype = f.subtype.to_string();
        c.structure_only = f.structure_only;
        c.cn0_dbhz = f.esn0_db + 10.0 * c.symbol_rate.max(1.0).log10();
        c.last_seen = now;
        let mut lines = Vec::new();
        for content in &f.contents {
            c.connection_pdus += u64::from(content.connection);
            c.addressed_pdus += u64::from(content.addressed);
            for s in &content.sdus {
                match s {
                    Sdu::Bulletin(b) => {
                        c.rnc = Some(b.rnc);
                        c.bct_id = Some(b.bct_id);
                        c.f_bearer = Some(b.f_bearer);
                        c.frame_no = Some(b.frame);
                        if b.spot.is_some() {
                            c.spot = b.spot;
                        }
                        let mut text = format!(
                            "bulletin rnc {} net-ver {} bearer {} bct-id {}",
                            b.rnc, b.net_ver, b.f_bearer, b.bct_id
                        );
                        if let Some(s) = b.spot {
                            text.push_str(&format!(" spot {s}"));
                        }
                        let mut first_sat = true;
                        for a in &b.avps {
                            avp_text(a, &mut text);
                            apply_avp(c, a, &mut first_sat, &mut self.beams, &mut self.networks, &mut self.satellites);
                        }
                        lines.push((format!("frame {} ", b.frame), text));
                    }
                    Sdu::AvpList(list) => {
                        let mut text = String::from("AVP list");
                        let mut first_sat = c.satellite.is_none();
                        for a in list {
                            avp_text(a, &mut text);
                            apply_avp(c, a, &mut first_sat, &mut self.beams, &mut self.networks, &mut self.satellites);
                        }
                        lines.push((String::new(), text));
                    }
                    Sdu::ReturnSchedule { channel, plans } => {
                        let short: u32 = plans.iter().map(|&p| u32::from(tables::PLAN_SHORT_RESERVED[usize::from(p & 15)])).sum();
                        let slots: u32 = plans.iter().map(|&p| u32::from(tables::PLAN_RESERVED[usize::from(p & 15)])).sum();
                        let share = 100.0 * f64::from(short) / (8.0 * plans.len().max(1) as f64);
                        let freq = tables::return_hz(*channel)
                            .map_or("unavailable".to_string(), |h| format!("{:.4} MHz", h / 1e6));
                        lines.push((
                            String::new(),
                            format!("return schedule {freq}, {share:.0} % reserved, {slots} connection slots"),
                        ));
                    }
                    Sdu::SpotBeamMap { version, number, spots } => lines.push((
                        String::new(),
                        format!("spot beam map version {version} number {number}, {spots} spots"),
                    )),
                    Sdu::BearerTables(n) => lines.push((String::new(), format!("bearer tables, {n} octets"))),
                    Sdu::GpsEphemeris { prn } => lines.push((String::new(), format!("GPS ephemeris PRN {prn}"))),
                    Sdu::SystemInfoIndex(items) => {
                        let hex: Vec<String> = items.iter().map(|b| format!("{b:02X}")).collect();
                        lines.push((String::new(), format!("system info index {}", hex.join(" "))));
                    }
                    Sdu::Other { kind, len } => lines.push((
                        String::new(),
                        format!("{} SDU of {len} octets", pdu::sdu_name(*kind)),
                    )),
                }
            }
        }
        let key: String = lines.iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>().join("; ");
        if !key.is_empty() && key != c.last_line {
            c.last_line = key;
            let freq = if self.tuned_hz > 0.0 {
                format!("{:.4} MHz", c.freq_hz / 1e6)
            } else {
                format!("{:+.1} kHz", c.offset_hz / 1e3)
            };
            for (prefix, t) in lines {
                let line = format!("{freq} {}: {prefix}{t}", c.bearer);
                self.fresh.push(line.clone());
                self.log.push((now, line));
            }
            if self.log.len() > LOG_LINES {
                let d = self.log.len() - LOG_LINES;
                self.log.drain(..d);
            }
        }
    }
}

/// The rate of the filter output over the symbol rate.
const SPS_RATE: f64 = super::demod::SPS;

/// Appends the plain-language text of an AVP.
fn avp_text(a: &Avp, t: &mut String) {
    use std::fmt::Write;
    let _ = match a {
        Avp::SatelliteLocation { sat, lon, .. } => {
            write!(t, ", satellite {sat} at {}", lon_text(f64::from(*lon) / 10.0))
        }
        Avp::BeamInfo { beam_type, spot } => {
            write!(t, ", beam {} spot {spot}", pdu::beam_type_name(*beam_type))
        }
        Avp::Plmn { mcc, mnc, .. } => write!(t, ", network {mcc}-{mnc}"),
        Avp::PrimaryBearer { spot, channel, bearer_type, .. } => write!(
            t,
            ", primary bearer of spot {spot} at {:.4} MHz {}",
            tables::forward_hz(*channel) / 1e6,
            tables::fbearer_type_name(*bearer_type)
        ),
        Avp::UtcTime { year, month, day, hour, minute, second, .. } => write!(
            t,
            ", GPS time {year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"
        ),
        Avp::StateVectors { xyz, .. } => write!(
            t,
            ", position {:.0} {:.0} {:.0} km",
            f64::from(xyz[0]) * 0.488,
            f64::from(xyz[1]) * 0.488,
            f64::from(xyz[2]) * 0.488
        ),
        Avp::CodeRates(list) => write!(t, ", code rates of {} blocks", list.len()),
        Avp::NasSystemInfo(n) => write!(t, ", NAS system info of {n} octets"),
        other => write!(t, ", {}", pdu::avp_name(other)),
    };
}

/// A longitude as text: "24.9 E", "98.0 W".
pub fn lon_text(lon: f64) -> String {
    if lon < 0.0 {
        format!("{:.1} W", -lon)
    } else {
        format!("{lon:.1} E")
    }
}

/// Applies an AVP to a carrier and to the band's tables.
fn apply_avp(
    c: &mut Carrier,
    a: &Avp,
    first_sat: &mut bool,
    beams: &mut BTreeMap<u8, Beam>,
    networks: &mut BTreeSet<String>,
    satellites: &mut Vec<SatPos>,
) {
    match a {
        Avp::SatelliteLocation { sat, lon, .. } => {
            if (-1800..=1799).contains(lon) {
                let p = SatPos {
                    id: *sat,
                    lon_deg: f64::from(*lon) / 10.0,
                };
                if *first_sat {
                    c.satellite = Some(p);
                    *first_sat = false;
                }
                if !satellites.iter().any(|s| s.id == p.id && (s.lon_deg - p.lon_deg).abs() < 0.05) {
                    satellites.push(p);
                }
            }
        }
        Avp::BeamInfo { beam_type, spot } => {
            c.beam_type = Some(*beam_type);
            c.spot = Some(*spot);
            beams.entry(*spot).or_default().beam_type = Some(*beam_type);
        }
        Avp::Plmn { mcc, mnc, .. } => {
            c.network = format!("{mcc}-{mnc}");
            networks.insert(c.network.clone());
        }
        Avp::PrimaryBearer { spot, channel, bearer_type, .. } => {
            let b = beams.entry(*spot).or_default();
            b.primary_hz = Some(tables::forward_hz(*channel));
            b.bearer_type = Some(*bearer_type);
        }
        _ => {}
    }
}

/// Whether `x` (a carrier at `rate` samples per second) shows the two lines of filtered OQPSK at [`GMR2_RATE`] in its squared
/// spectrum: a pair of lines one or two symbol rates apart, centred within 8 kHz of 0 Hz.
pub fn gmr2_lines(x: &[Complex32], rate: f64) -> bool {
    let n = 8192usize;
    if x.len() < 2 * n || rate < 2.4 * GMR2_RATE {
        return false;
    }
    let mut fft = Fft::forward(n);
    let mut acc = vec![0f32; n];
    let mut buf = vec![Complex32::default(); n];
    let segs = (x.len() / n).min(8);
    let start = x.len() - segs * n;
    for s in 0..segs {
        for (b, v) in buf.iter_mut().zip(&x[start + s * n..start + (s + 1) * n]) {
            *b = v * v;
        }
        fft.process(&mut buf);
        for (a, b) in acc.iter_mut().zip(&buf) {
            *a += b.norm_sqr();
        }
    }
    let bin = rate / n as f64;
    let mut scratch = Vec::new();
    let median = percentile(&acc, 0.5, &mut scratch).max(1e-30);
    let freq = |k: usize| if k >= n / 2 { k as f64 - n as f64 } else { k as f64 } * bin;
    let index = |hz: f64| (hz / bin).round().rem_euclid(n as f64) as usize;
    let line = |centre: f64| -> Option<f64> {
        let lo = index(centre - 8e3);
        let w = (16e3 / bin) as usize;
        let k = (0..=w).map(|j| (lo + j) % n).max_by(|&a, &b| acc[a].total_cmp(&acc[b]))?;
        (acc[k] / median > GMR2_LINE).then(|| freq(k))
    };
    for sep in [GMR2_RATE, 2.0 * GMR2_RATE] {
        if let (Some(a), Some(b)) = (line(-sep / 2.0), line(sep / 2.0)) {
            if ((b - a) - sep).abs() < 2e3 {
                return true;
            }
        }
    }
    false
}
