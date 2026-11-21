//! The readings of one second of an AM station at the working rate ([`AmMeter`]) and the carrier and over-modulation rules
//! that turn them into changes ([`AmState`]).

use crate::instrument::{Held, Spectrum};
use sdrkit_algo_dsp::blockrank::{BlockRank, BlockValues};
use sdrkit_algo_dsp::carrierline::CarrierLine;
use sdrkit_core::Complex32;
use std::f64::consts::TAU;

/// The carrier test: the share of a block's power in its strongest 5 adjacent bins must exceed this.
pub const CARRIER_SHARE: f64 = 0.012;
/// The bins of the carrier test run.
pub const CARRIER_BINS: usize = 5;
/// The mains hum frequencies searched, Hz.
pub const HUM_HZ: [f64; 4] = [50.0, 100.0, 60.0, 120.0];
/// A block of the peak reader, s.
pub const BLOCK_S: f64 = 0.02;
/// A block counts as at zero carrier when its third smallest envelope value is under this share of the mean.
pub const ZERO_SHARE: f64 = 0.03;
/// The share of the audio power under the audio bandwidth.
pub const BANDWIDTH_SHARE: f64 = 0.99;

/// The readings of one second.
#[derive(Clone, Debug, PartialEq)]
pub struct Second {
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The carrier level, dB (relative): `10 log10(M^2)` of the mean envelope `M`.
    pub carrier_db: f64,
    /// The carrier test passed and the mean envelope is over 1e-6.
    pub present: bool,
    /// The share of the last carrier test block.
    pub line_share: f64,
    /// The modulation depth (RMS, as a sine), %.
    pub depth_pct: f64,
    /// The positive peak, %.
    pub peak_pos_pct: f64,
    /// The negative peak, % (at most 100).
    pub peak_neg_pct: f64,
    /// The asymmetry, %.
    pub asymmetry_pct: f64,
    /// The share of the second in blocks at zero carrier, %.
    pub over_mod_pct: f64,
    /// The mains hum: (frequency Hz, dB below the carrier); `None` when not seen.
    pub hum: Option<(f64, f64)>,
    /// The audio bandwidth, Hz; `None` without audio power.
    pub bandwidth_hz: Option<f64>,
}

/// Measures one second at a time.
#[derive(Debug)]
pub struct AmMeter {
    rate: f64,
    per_second: usize,
    env: Vec<f32>,
    steps: (f64, f64),
    prev: Option<Complex32>,
    rank: BlockRank,
    blocks: Vec<BlockValues>,
    line: CarrierLine,
    spectrum: Spectrum,
    audio: Vec<f32>,
}

impl AmMeter {
    /// A meter at the working rate `rate_hz`.
    pub fn new(rate_hz: f64) -> AmMeter {
        let per_second = rate_hz.round().max(1.0) as usize;
        AmMeter {
            rate: rate_hz,
            per_second,
            env: Vec::with_capacity(per_second),
            steps: (0.0, 0.0),
            prev: None,
            rank: BlockRank::new(((BLOCK_S * rate_hz).round() as usize).max(8), 3),
            blocks: Vec::with_capacity(64),
            line: CarrierLine::new(CarrierLine::block_len(rate_hz, 0.4, 256, 32768), CARRIER_BINS),
            spectrum: Spectrum::new(rate_hz, 5.0),
            audio: Vec::with_capacity(per_second),
        }
    }

    /// The working rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Takes samples; calls `done` with the readings of each completed second.
    pub fn push(&mut self, x: &[Complex32], mut done: impl FnMut(Second)) {
        let mut rest = x;
        while !rest.is_empty() {
            let take = (self.per_second - self.env.len()).min(rest.len());
            let (now, later) = rest.split_at(take);
            self.line.push(now);
            for &a in now {
                let e = a.norm();
                self.env.push(e);
                if let Some(b) = self.rank.push(f64::from(e)) {
                    self.blocks.push(b);
                }
                if let Some(p) = self.prev {
                    let s = a * p.conj();
                    self.steps.0 += f64::from(s.re);
                    self.steps.1 += f64::from(s.im);
                }
                self.prev = Some(a);
            }
            rest = later;
            if self.env.len() == self.per_second {
                done(self.second());
            }
        }
    }

    /// Forgets the second under way.
    pub fn reset(&mut self) {
        self.env.clear();
        self.blocks.clear();
        self.rank.reset();
        self.steps = (0.0, 0.0);
        self.prev = None;
        self.line.reset();
    }

    fn second(&mut self) -> Second {
        let n = self.env.len().max(1) as f64;
        let m = self.env.iter().map(|&e| f64::from(e)).sum::<f64>() / n;
        let var = self.env.iter().map(|&e| (f64::from(e) - m).powi(2)).sum::<f64>() / n;
        let line_share = self.line.share().unwrap_or(0.0);
        let present = m > 1e-6 && line_share > CARRIER_SHARE;
        let mm = m.max(1e-30);
        let high = self.blocks.iter().map(|b| b.high).fold(f64::NEG_INFINITY, f64::max);
        let low = self.blocks.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let (pos, neg) = if self.blocks.is_empty() {
            (0.0, 0.0)
        } else {
            ((high - m) / mm, ((m - low) / mm).min(1.0))
        };
        let asym = if pos + neg > 0.0 { (pos - neg) / (pos + neg) } else { 0.0 };
        let zero: usize = self
            .blocks
            .iter()
            .filter(|b| b.low < ZERO_SHARE * m)
            .map(|b| b.len)
            .sum();
        self.audio.clear();
        self.audio.extend(self.env.iter().map(|&e| (f64::from(e) - m) as f32));
        let (hum, bandwidth_hz) = if self.spectrum.compute(&self.audio) {
            (self.hum(m), self.bandwidth())
        } else {
            (None, None)
        };
        let offset_hz = self.steps.1.atan2(self.steps.0) * self.rate / TAU;
        let s = Second {
            offset_hz,
            carrier_db: 10.0 * (m * m).max(1e-30).log10(),
            present,
            line_share,
            depth_pct: 100.0 * std::f64::consts::SQRT_2 * var.sqrt() / mm,
            peak_pos_pct: 100.0 * pos,
            peak_neg_pct: 100.0 * neg,
            asymmetry_pct: 100.0 * asym,
            over_mod_pct: 100.0 * zero as f64 / n,
            hum,
            bandwidth_hz,
        };
        self.env.clear();
        self.blocks.clear();
        self.rank.reset();
        self.steps = (0.0, 0.0);
        s
    }

    /// The strongest mains line of [`HUM_HZ`] when it stands 8 times over the audio around it.
    fn hum(&self, m: f64) -> Option<(f64, f64)> {
        let s = &self.spectrum;
        let bin = s.bin_hz();
        let c = m * m / 2.0;
        let (f, p) = HUM_HZ
            .iter()
            .map(|&f| (f, s.band(f - 3.0 * bin, f + 3.0 * bin)))
            .fold((0.0, -1.0), |best, x| if x.1 > best.1 { x } else { best });
        let around = 0.5 * (s.band((f - 40.0).max(20.0), f - 15.0) + s.band(f + 15.0, f + 40.0)) * (6.0 * bin) / 25.0;
        (p > 8.0 * around && p > 1e-8 * c).then(|| (f, -10.0 * (p / c).log10()))
    }

    /// The frequency under which 99 % of the audio power lies, from 30 Hz up.
    fn bandwidth(&self) -> Option<f64> {
        let s = &self.spectrum;
        let bin = s.bin_hz();
        let top = 12_000f64.min(self.rate / 2.0 - 100.0);
        let total = s.band(30.0, top);
        if total <= 0.0 {
            return None;
        }
        let first = (30.0 / bin).floor() as usize;
        let last = ((top / bin).ceil() as usize).min(s.bins().len() - 1);
        let mut sum = 0.0;
        for k in first..=last {
            sum += s.bins()[k];
            if sum >= BANDWIDTH_SHARE * total {
                return Some(k as f64 * bin);
            }
        }
        Some(last as f64 * bin)
    }
}

/// A change of the carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CarrierChange {
    /// The first carrier.
    Present,
    /// The carrier went away.
    Lost,
    /// The carrier came back.
    Returned,
}

/// What a second changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// The carrier changed.
    pub carrier: Option<CarrierChange>,
    /// The second was over-modulated.
    pub over_modulated: bool,
}

/// The carrier state, the held level and the over-modulation count.
#[derive(Clone, Debug, Default)]
pub struct AmState {
    held: Held,
    level: f64,
    ever_on: bool,
    /// Seconds over-modulated.
    pub over_mod_s: u64,
}

impl AmState {
    /// Whether the carrier is present.
    pub fn on(&self) -> bool {
        self.held.get() == Some(true)
    }

    /// Takes one second: the carrier is lost below the held level by `loss_db`; a second is over-modulated above
    /// `share_pct` % at zero carrier while the carrier test passes.
    pub fn push(&mut self, s: &Second, loss_db: f64, share_pct: f64) -> Changes {
        let first = self.held.get().is_none();
        let wanted = match self.held.get() {
            None => s.present,
            Some(true) => s.present && s.carrier_db >= self.level - loss_db,
            Some(false) => s.present && s.carrier_db >= self.level - 6.0,
        };
        if first {
            self.level = s.carrier_db;
        }
        let mut c = Changes::default();
        if let Some(now) = self.held.push(wanted) {
            c.carrier = match (now, first, self.ever_on) {
                (true, _, false) => Some(CarrierChange::Present),
                (true, _, true) => Some(CarrierChange::Returned),
                (false, true, _) => None,
                (false, false, _) => Some(CarrierChange::Lost),
            };
            self.ever_on |= now;
        }
        if self.on() {
            self.level = s.carrier_db.max(self.level - 0.05);
        }
        if s.present && s.over_mod_pct > share_pct {
            self.over_mod_s += 1;
            c.over_modulated = true;
        }
        c
    }
}
