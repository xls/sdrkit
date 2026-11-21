//! The NavIC receiver core on the shared parts of the crate: the band resampler to 4.096 Msps, the budgeted code search over
//! PRN 1 to 14 with a confirmation, and the tracking channels with the NavIC message layer.

use super::code::{ranging_code, Band, Subframe, L5_HZ, MAX_PRN};
use crate::acq::{AcqHit, Integration, Search, Searcher};
use crate::codes::SignalKind;
use crate::frontend::{BandBuffer, BandResampler, SinCos};
use crate::msg::{Layer, NavUnit};
use crate::system::{SatId, System};
use crate::track::{ChanEvent, Channel, ChannelState};
use sdrkit_core::Complex32;
use std::collections::BTreeMap;

/// The most channels.
pub const MAX_CHANNELS: usize = 8;
/// The Doppler search range, Hz.
const DOPPLER_HZ: f64 = 10e3;
/// Code periods of a search segment.
const PERIODS: usize = 10;
/// Signal seconds one PRN's search is spread over.
const SEARCH_S: f64 = 0.2;
/// A subframe within this time keeps the lamp at Locked, s.
pub const LOCK_HOLD_S: f64 = 30.0;

/// What the stack knows of one satellite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sat {
    /// The PRN.
    pub prn: u16,
    /// 1 carrier lock, 2 bit sync, 3 subframes decoded; 0 pulling in.
    pub state: u8,
    /// C/N0, dB-Hz.
    pub cn0: f64,
    /// Doppler, Hz.
    pub doppler_hz: f64,
    /// Subframes good.
    pub good: u64,
    /// Subframes bad.
    pub bad: u64,
    /// The last subframe.
    pub last: Option<Subframe>,
    /// The week number of the last subframe 1.
    pub wn: Option<u16>,
    /// Signal time of the last good subframe, s.
    pub last_good_s: f64,
    /// Tracked now.
    pub tracked: bool,
}

/// A good subframe.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard {
    /// The PRN.
    pub prn: u16,
    /// The fields.
    pub subframe: Subframe,
    /// Signal time, s.
    pub time_s: f64,
}

/// The receiver core.
pub struct NavicCore {
    rate: f64,
    resampler: BandResampler,
    lut: SinCos,
    band: BandBuffer,
    tmp: Vec<Complex32>,
    searcher: Searcher,
    codes: Vec<Vec<u8>>,
    job: Option<Search>,
    spare: Option<Search>,
    confirming: Option<AcqHit>,
    next_prn: u16,
    /// The PRN being searched (0 when none).
    pub searching: u16,
    channels: Vec<Channel>,
    /// What is known of each satellite, by PRN.
    pub sats: BTreeMap<u16, Sat>,
    /// Good subframes since the last call of [`NavicCore::take_heard`].
    heard: Vec<Heard>,
    /// Log lines since the last call of [`NavicCore::take_log`].
    log: Vec<String>,
    /// Signal time, s.
    pub now_s: f64,
    events: Vec<ChanEvent>,
}

impl NavicCore {
    /// A core for an input at `rate` with the carrier `offset_hz` from its centre.
    pub fn new(rate: f64, offset_hz: f64) -> NavicCore {
        let kind = SignalKind::L1Ca;
        let mut searcher = Searcher::new(kind);
        let codes: Vec<Vec<u8>> = (1..=MAX_PRN)
            .map(|p| ranging_code(p, Band::L5).unwrap_or_default())
            .collect();
        for (i, c) in codes.iter().enumerate() {
            searcher.add_code(i as u16 + 1, c);
        }
        NavicCore {
            rate,
            resampler: BandResampler::new(rate, kind.band_rate(), offset_hz),
            lut: SinCos::new(),
            band: BandBuffer::default(),
            tmp: Vec::new(),
            searcher,
            codes,
            job: None,
            spare: None,
            confirming: None,
            next_prn: 1,
            searching: 0,
            channels: Vec::with_capacity(MAX_CHANNELS),
            sats: BTreeMap::new(),
            heard: Vec::new(),
            log: Vec::new(),
            now_s: 0.0,
            events: Vec::new(),
        }
    }

    /// The good subframes since the last call.
    pub fn take_heard(&mut self) -> Vec<Heard> {
        std::mem::take(&mut self.heard)
    }

    /// The log lines since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }

    /// Channels tracked now.
    pub fn tracked(&self) -> usize {
        self.channels.len()
    }

    /// Takes input samples.
    pub fn feed(&mut self, x: &[Complex32]) {
        self.now_s += x.len() as f64 / self.rate;
        self.tmp.clear();
        self.resampler.process(x, &self.lut, &mut self.tmp);
        self.band.data.extend_from_slice(&self.tmp);
        self.search(x.len());
        self.track();
        let n = SignalKind::L1Ca.period_samples();
        let keep_from = self
            .channels
            .iter()
            .map(|c| c.needs_from())
            .min()
            .unwrap_or(u64::MAX);
        let search_from = self.band.end().saturating_sub(((PERIODS + 2) * n) as u64);
        self.band
            .trim(keep_from.min(search_from), (PERIODS + 2) * n);
    }

    fn search(&mut self, input: usize) {
        let kind = SignalKind::L1Ca;
        let n = kind.period_samples();
        if self.job.is_none() {
            let end = self.band.end();
            let Some(seg_start) = end.checked_sub((PERIODS * n) as u64) else {
                return;
            };
            let Some(seg) = self.band.slice(seg_start, PERIODS * n) else {
                return;
            };
            let integ = Integration {
                periods: PERIODS,
                coherent: 1,
            };
            if let Some(hit) = &self.confirming {
                self.job = Some(self.searcher.start_confirm(
                    hit.prn,
                    seg,
                    seg_start,
                    hit.doppler_hz,
                    integ,
                    L5_HZ,
                    0.0,
                    self.spare.take(),
                ));
            } else {
                let tracked: Vec<u16> = self.channels.iter().map(|c| c.sat.prn).collect();
                if self.channels.len() >= MAX_CHANNELS || tracked.len() >= MAX_PRN as usize {
                    self.searching = 0;
                    return;
                }
                let mut prn = self.next_prn;
                while tracked.contains(&prn) {
                    prn = prn % MAX_PRN + 1;
                }
                self.next_prn = prn % MAX_PRN + 1;
                self.searching = prn;
                self.job = Some(self.searcher.start(
                    prn,
                    seg,
                    seg_start,
                    0.0,
                    DOPPLER_HZ,
                    4,
                    integ,
                    L5_HZ,
                    0.0,
                    1e-3,
                    self.spare.take(),
                ));
            }
        }
        let Some(job) = self.job.as_mut() else {
            return;
        };
        // A stage of the search runs when the budget covers it, or 8 units of a dearer stage.
        let mut budget = (job.cost() * input as f64 / (SEARCH_S * self.rate)).max(8.0);
        let Some((hit, _)) = self.searcher.step(job, &mut budget) else {
            return;
        };
        self.spare = self.job.take();
        match self.confirming.take() {
            None if hit.found => {
                self.log.push(format!(
                    "PRN {} found: Doppler {:+.0} Hz, peak {:.1} times the noise",
                    hit.prn, hit.doppler_hz, hit.ratio
                ));
                self.confirming = Some(hit);
            }
            Some(_) if hit.found => self.start_channel(&hit),
            _ => {}
        }
    }

    fn start_channel(&mut self, hit: &AcqHit) {
        if self.channels.iter().any(|c| c.sat.prn == hit.prn) {
            return;
        }
        let chips = &self.codes[usize::from(hit.prn - 1)];
        let mut ch = Channel::start(
            SatId::new(System::Gps, hit.prn),
            SignalKind::L1Ca,
            L5_HZ,
            chips,
            hit,
            self.band.end(),
            0.0,
            0.0,
            self.now_s,
        );
        ch.layer = Layer::Navic(Box::default());
        self.channels.push(ch);
        self.sats.entry(hit.prn).or_insert_with(|| Sat {
            prn: hit.prn,
            ..Sat::default()
        });
    }

    fn track(&mut self) {
        let mut keep = Vec::with_capacity(self.channels.len());
        for mut ch in self.channels.drain(..) {
            self.events.clear();
            let alive = ch.run(&self.band, &self.lut, self.now_s, &mut self.events);
            let prn = ch.sat.prn;
            let sat = self.sats.entry(prn).or_default();
            sat.prn = prn;
            sat.cn0 = ch.cn0.value;
            sat.doppler_hz = ch.carr_freq;
            sat.tracked = alive;
            sat.state = match ch.state {
                ChannelState::PullIn => 0,
                ChannelState::Locked => 1,
                ChannelState::BitSync => 2,
                ChannelState::FrameSync | ChannelState::Ephemeris => 3,
            };
            for e in self.events.drain(..) {
                match e {
                    ChanEvent::Unit {
                        unit: NavUnit::Navic(sf),
                        ..
                    } => {
                        sat.good += 1;
                        sat.last_good_s = self.now_s;
                        if sf.id == 1 {
                            sat.wn = sf.wn;
                        }
                        sat.last = Some(sf.clone());
                        self.heard.push(Heard {
                            prn,
                            subframe: sf,
                            time_s: self.now_s,
                        });
                    }
                    ChanEvent::Bad => sat.bad += 1,
                    ChanEvent::Lost { reason, .. } => {
                        self.log.push(format!("PRN {prn} lost: {reason}"));
                    }
                    ChanEvent::Log(l) => self.log.push(format!("PRN {prn}: {l}")),
                    _ => {}
                }
            }
            if alive {
                keep.push(ch);
            }
        }
        self.channels = keep;
    }
}
