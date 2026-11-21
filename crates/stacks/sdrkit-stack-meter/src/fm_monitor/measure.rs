//! The readings of one second of the multiplex (MPX) of an FM station ([`FmMeter`]) and the state rules that turn them into
//! changes ([`FmState`]).

use crate::instrument::{Held, Spectrum};
use sdrkit_algo_dsp::blockrank::BlockRank;
use sdrkit_core::Complex32;
use std::collections::VecDeque;
use std::f64::consts::TAU;

/// The pilot frequency, Hz (ITU-R BS.450).
pub const PILOT_HZ: f64 = 19_000.0;
/// The RDS subcarrier, Hz (IEC 62106, ITU-R BS.643).
pub const RDS_HZ: f64 = 57_000.0;
/// 0 dBr of the MPX power: the power of a sine of 19 kHz deviation (ITU-R BS.412), Hz^2.
pub const MPX_REFERENCE: f64 = 19_000.0 * 19_000.0 / 2.0;
/// The weight of a sample in the running mean of the excursion.
pub const MEAN_WEIGHT: f64 = 2e-5;
/// A block of the peak reader, s.
pub const BLOCK_S: f64 = 0.005;
/// The window of the floating MPX power, s (ITU-R BS.412).
pub const MPX_WINDOW_S: usize = 60;
/// The least working rate at which the 92 kHz subcarrier is read, Hz.
pub const SCA92_RATE_HZ: f64 = 250e3;
/// The RDS band, Hz either side of [`RDS_HZ`].
pub const RDS_HALF_HZ: f64 = 2400.0;
/// The band above RDS that holds only noise, Hz.
pub const RDS_ABOVE_HZ: (f64, f64) = (60_000.0, 62_500.0);
/// RDS counts when the power per Hz of its band is this many times that of [`RDS_ABOVE_HZ`]. Noise alone after an FM
/// discriminator gives about 0.87, since its power per Hz rises with the square of the frequency.
pub const RDS_OVER: f64 = 2.0;

/// The readings of one second.
#[derive(Clone, Debug, PartialEq)]
pub struct Second {
    /// The peak deviation: the largest third-largest excursion of the 5 ms blocks, Hz.
    pub peak_hz: f64,
    /// The RMS deviation, Hz.
    pub rms_hz: f64,
    /// The variance of the MPX, Hz^2.
    pub variance: f64,
    /// The carrier offset, Hz.
    pub offset_hz: f64,
    /// The audio (L+R), peak equivalent, Hz.
    pub audio_hz: f64,
    /// The pilot, peak equivalent, Hz.
    pub pilot_hz: f64,
    /// The pilot stands 3 times over the bands either side and over 2 kHz.
    pub pilot: bool,
    /// The stereo (L-R) band, peak equivalent, Hz.
    pub stereo_hz: f64,
    /// The RDS injection, peak equivalent, Hz.
    pub rds_hz: f64,
    /// RDS is over 0.3 kHz and its power per Hz is [`RDS_OVER`] times that of the band above it.
    pub rds: bool,
    /// The 67 kHz subcarrier over the floor, dB.
    pub sca67_db: f64,
    /// The 67 kHz subcarrier is present.
    pub sca67: bool,
    /// The 92 kHz subcarrier over the floor, dB; `None` below a working rate of 250 ksps.
    pub sca92_db: Option<f64>,
    /// The 92 kHz subcarrier is present.
    pub sca92: bool,
    /// The input level, dB.
    pub level_db: f64,
}

impl Second {
    /// The MPX power of this second, dBr.
    pub fn mpx_dbr(&self) -> f64 {
        10.0 * (self.variance / MPX_REFERENCE).max(1e-30).log10()
    }

    /// Stereo: the pilot is present and the L-R band is over 0.5 kHz.
    pub fn stereo(&self) -> bool {
        self.pilot && self.stereo_hz > 500.0
    }
}

/// Measures one second at a time.
#[derive(Debug)]
pub struct FmMeter {
    rate: f64,
    per_second: usize,
    f: Vec<f32>,
    prev: Option<Complex32>,
    mean: Option<f64>,
    rank: BlockRank,
    peak: f64,
    power: f64,
    spectrum: Spectrum,
}

impl FmMeter {
    /// A meter at the working rate `rate_hz`.
    pub fn new(rate_hz: f64) -> FmMeter {
        let per_second = rate_hz.round().max(1.0) as usize;
        FmMeter {
            rate: rate_hz,
            per_second,
            f: Vec::with_capacity(per_second),
            prev: None,
            mean: None,
            rank: BlockRank::new(((BLOCK_S * rate_hz).round() as usize).max(8), 3),
            peak: 0.0,
            power: 0.0,
            spectrum: Spectrum::new(rate_hz, 30.0),
        }
    }

    /// Forgets the second under way.
    pub fn reset(&mut self) {
        self.f.clear();
        self.prev = None;
        self.mean = None;
        self.rank.reset();
        self.peak = 0.0;
        self.power = 0.0;
    }

    /// Takes samples; calls `done` with the readings of each completed second.
    pub fn push(&mut self, x: &[Complex32], mut done: impl FnMut(Second)) {
        let k = self.rate / TAU;
        for &a in x {
            self.power += f64::from(a.norm_sqr());
            let Some(p) = self.prev.replace(a) else {
                continue;
            };
            let s = a * p.conj();
            let f = f64::from(s.im).atan2(f64::from(s.re)) * k;
            let m = self.mean.get_or_insert(f);
            *m += MEAN_WEIGHT * (f - *m);
            if let Some(b) = self.rank.push((f - *m).abs()) {
                self.peak = self.peak.max(b.high);
            }
            self.f.push(f as f32);
            if self.f.len() == self.per_second {
                done(self.second());
            }
        }
    }

    fn second(&mut self) -> Second {
        let n = self.f.len().max(1) as f64;
        let mean = self.f.iter().map(|&v| f64::from(v)).sum::<f64>() / n;
        let variance = self.f.iter().map(|&v| (f64::from(v) - mean).powi(2)).sum::<f64>() / n;
        for v in self.f.iter_mut() {
            *v -= mean as f32;
        }
        let ok = self.spectrum.compute(&self.f);
        let s = &self.spectrum;
        let b = s.bin_hz();
        let pilot_hz = s.amplitude(PILOT_HZ - 3.0 * b, PILOT_HZ + 3.0 * b);
        let sides = 0.5
            * (s.amplitude(PILOT_HZ - 12.0 * b, PILOT_HZ - 6.0 * b) + s.amplitude(PILOT_HZ + 6.0 * b, PILOT_HZ + 12.0 * b));
        let rds_hz = s.amplitude(RDS_HZ - RDS_HALF_HZ, RDS_HZ + RDS_HALF_HZ);
        let rds_density = s.band(RDS_HZ - RDS_HALF_HZ, RDS_HZ + RDS_HALF_HZ) / (2.0 * RDS_HALF_HZ);
        let above_density = s.band(RDS_ABOVE_HZ.0, RDS_ABOVE_HZ.1) / (RDS_ABOVE_HZ.1 - RDS_ABOVE_HZ.0);
        let floor = self.floor();
        let level = |f0: f64, f1: f64, bands: f64| 10.0 * (s.band(f0, f1) / (floor * bands).max(1e-30)).max(1e-30).log10();
        let sca67_db = level(63_000.0, 71_000.0, 4.0);
        let sca67 = ok && sca67_db > 10.0 && s.amplitude(63_000.0, 71_000.0) > 300.0;
        let sca92_db = (self.rate >= SCA92_RATE_HZ && self.rate / 2.0 > 97_000.0).then(|| level(87_000.0, 97_000.0, 5.0));
        let sca92 = ok && sca92_db.is_some_and(|d| d > 10.0) && s.amplitude(87_000.0, 97_000.0) > 300.0;
        let out = Second {
            peak_hz: self.peak,
            rms_hz: variance.sqrt(),
            variance,
            offset_hz: mean,
            audio_hz: s.amplitude(30.0, 15_000.0),
            pilot_hz,
            pilot: ok && pilot_hz > 2000.0 && pilot_hz > 3.0 * sides,
            stereo_hz: s.amplitude(23_000.0, 53_000.0),
            rds_hz,
            rds: ok && rds_hz > 300.0 && rds_density > RDS_OVER * above_density,
            sca67_db,
            sca67,
            sca92_db,
            sca92,
            level_db: 10.0 * (self.power / n).max(1e-30).log10(),
        };
        self.f.clear();
        self.peak = 0.0;
        self.power = 0.0;
        out
    }

    /// The floor of the subcarriers: the median of the 2 kHz bands from 60 kHz up to `min(100 kHz, rate / 2 - 2 kHz)`.
    fn floor(&self) -> f64 {
        let top = 100_000f64.min(self.rate / 2.0 - 2000.0);
        let mut bands = Vec::with_capacity(24);
        let mut f0 = 60_000.0;
        while f0 + 2000.0 <= top {
            bands.push(self.spectrum.band(f0, f0 + 2000.0));
            f0 += 2000.0;
        }
        if bands.is_empty() {
            return 0.0;
        }
        bands.sort_by(f64::total_cmp);
        bands[bands.len() / 2]
    }
}

/// A change of the pilot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PilotChange {
    /// The pilot is present (at the first measurement, or back).
    Present,
    /// No pilot at the first measurement.
    Absent,
    /// The pilot went away.
    Lost,
}

/// What a second changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Changes {
    /// The pilot changed.
    pub pilot: Option<PilotChange>,
    /// RDS came (true) or went (false).
    pub rds: Option<bool>,
    /// The 67 kHz subcarrier came or went.
    pub sca67: Option<bool>,
    /// The 92 kHz subcarrier came or went.
    pub sca92: Option<bool>,
    /// The peak was over the threshold.
    pub over_deviation: bool,
}

/// The states of the pilot, RDS and the subcarriers, the over-deviation count and the floating MPX power.
#[derive(Clone, Debug, Default)]
pub struct FmState {
    pilot: Held,
    rds: Held,
    sca67: Held,
    sca92: Held,
    /// Seconds over the threshold.
    pub over_deviation_s: u64,
    variances: VecDeque<f64>,
}

impl FmState {
    /// The pilot is present.
    pub fn pilot(&self) -> bool {
        self.pilot.get() == Some(true)
    }

    /// RDS is present.
    pub fn rds(&self) -> bool {
        self.rds.get() == Some(true)
    }

    /// The 67 kHz and the 92 kHz subcarriers are present.
    pub fn sca(&self) -> (bool, bool) {
        (self.sca67.get() == Some(true), self.sca92.get() == Some(true))
    }

    /// The MPX power over the last 60 s in steps of 1 s, dBr; `None` before 60 s were measured.
    pub fn mpx_60s_dbr(&self) -> Option<f64> {
        (self.variances.len() >= MPX_WINDOW_S).then(|| {
            let v = self.variances.iter().sum::<f64>() / self.variances.len() as f64;
            10.0 * (v / MPX_REFERENCE).max(1e-30).log10()
        })
    }

    /// Takes one second; a peak over `threshold_hz` is an over-deviation.
    pub fn push(&mut self, s: &Second, threshold_hz: f64) -> Changes {
        let first = self.pilot.get().is_none();
        let mut c = Changes {
            pilot: self.pilot.push(s.pilot).map(|now| match (now, first) {
                (true, _) => PilotChange::Present,
                (false, true) => PilotChange::Absent,
                (false, false) => PilotChange::Lost,
            }),
            rds: self.rds.push(s.rds).filter(|_| !first),
            sca67: self.sca67.push(s.sca67).filter(|_| !first),
            sca92: self.sca92.push(s.sca92).filter(|_| !first),
            over_deviation: false,
        };
        if s.peak_hz > threshold_hz {
            self.over_deviation_s += 1;
            c.over_deviation = true;
        }
        if self.variances.len() == MPX_WINDOW_S {
            self.variances.pop_front();
        }
        self.variances.push_back(s.variance);
        c
    }
}
