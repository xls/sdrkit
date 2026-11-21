//! The test signal of the lightning position stack: strikes at random times and places around a test station, heard by one
//! receiving station after the ground-wave delay, and an optional MSK timing transmitter.
//!
//! Times are true seconds from an epoch that the caller chooses; input sample `n` of the receiving station lies at true time
//! `start_s + n / (rate (1 + ppm 1e-6))`. Every station made with the same seed hears the same strikes.

use super::{GEN_STATION_LAT, GEN_STATION_LON, GEN_STRIKE_RATE, GEN_TIMING_OFFSET_HZ, GEN_TIMING_TX};
use sdrkit_algo_locate::propagation::{distance_m, Propagation};
use sdrkit_algo_modem::msktiming::MskSignal;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, ConfigError, GenConfig, Generator, Hz, SignalInfo};
use sdrkit_util_geo::Position;
use std::collections::VecDeque;
use std::f64::consts::{PI, TAU};

/// The centre of the strike area, degrees.
pub const TEST_STATION: (f64, f64) = (52.5, 13.4);
/// Strikes lie within this many degrees of latitude and of longitude from [`TEST_STATION`].
pub const STRIKE_SPAN_DEG: (f64, f64) = (3.0, 4.0);
/// The position of the test signal's timing transmitter, degrees.
pub const TIMING_TX: (f64, f64) = (52.0, 10.0);
/// The symbol rate of the test signal's timing transmitter, Bd.
pub const TIMING_BAUD: f64 = 200.0;
/// The amplitude of the timing transmitter.
pub const TIMING_AMPLITUDE: f64 = 0.05;
/// The raised-cosine rise of a stroke, s.
pub const RISE_S: f64 = 12e-6;
/// The decay time of a stroke, s.
pub const DECAY_S: f64 = 60e-6;
/// A stroke lasts its rise and this many decay times.
pub const DECAY_TAUS: f64 = 8.0;
/// The highest rotation of a stroke, Hz; at most an eighth of the rate.
pub const OSC_HZ: f64 = 25e3;
/// The amplitude of a stroke at 100 km lies in this span; it falls as one over the distance.
pub const AMP_100KM: (f64, f64) = (0.3, 0.8);
/// The largest stroke amplitude.
pub const AMP_MAX: f64 = 0.9;
/// The shortest gap between two strikes, s.
pub const MIN_GAP_S: f64 = 2e-3;
/// The first strike comes this long after the epoch, s: after the floor of the timer is valid.
pub const FIRST_STRIKE_S: f64 = 0.5;
/// The mean power declared for the SNR of the impairments: 30 dB gives noise of 0.01 rms over the whole rate.
pub const DECLARED_POWER: f64 = 0.1;

/// The rotation of a stroke at `rate`, Hz.
pub fn stroke_hz(rate: f64) -> f64 {
    OSC_HZ.min(rate / 8.0)
}

/// The length of a stroke, s.
pub fn stroke_s() -> f64 {
    RISE_S + DECAY_TAUS * DECAY_S
}

/// Adds to `x` one stroke whose onset lies at the fractional input index `onset`, in a stream whose first sample in `x` has
/// the index `first` and whose sample clock gives `true_rate` samples a true second. The envelope rises as a raised cosine
/// over [`RISE_S`] and decays with [`DECAY_S`]; the stroke rotates at `hz` from the phase `phase` (rad). The waveform is
/// evaluated at the true time of each sample, so the onset can fall between samples; the parts outside `x` are left out.
pub fn add_stroke(
    x: &mut [Complex32],
    first: u64,
    true_rate: f64,
    onset: f64,
    amp: f64,
    hz: f64,
    phase: f64,
) {
    let len = stroke_s() * true_rate;
    let lo = (onset.ceil().max(0.0) as u64).max(first);
    let hi = ((onset + len).ceil().max(0.0) as u64).min(first + x.len() as u64);
    for n in lo..hi {
        let t = (n as f64 - onset) / true_rate;
        let a = if t < RISE_S {
            0.5 * (1.0 - (PI * t / RISE_S).cos())
        } else {
            (-(t - RISE_S) / DECAY_S).exp()
        };
        let p = phase + TAU * (hz * t).fract();
        x[(n - first) as usize] += Complex32::from_polar((amp * a) as f32, p as f32);
    }
}

/// One strike of the test signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strike {
    /// The time of the strike, true seconds from the epoch.
    pub time_s: f64,
    /// Where it struck.
    pub position: Position,
    /// Its amplitude at 100 km.
    pub amp_100km: f64,
    /// The phase of its rotation, rad.
    pub phase: f64,
}

/// A strike as the receiving station hears it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    /// The strike.
    pub strike: Strike,
    /// The onset at the station: a fractional input sample index.
    pub onset: f64,
    /// The amplitude at the station.
    pub amplitude: f64,
}

/// The strikes of one seed, in time order.
#[derive(Clone, Debug)]
struct Strikes {
    rng: Rng,
    per_s: f64,
    next_s: f64,
}

impl Strikes {
    fn new(seed: u64, per_s: f64, first_s: f64) -> Self {
        Strikes {
            rng: Rng::new(seed),
            per_s,
            next_s: first_s,
        }
    }

    /// The next strike, or `None` when the rate is 0.
    fn next(&mut self) -> Option<Strike> {
        if !(self.per_s > 0.0 && self.per_s.is_finite()) {
            return None;
        }
        let (lat0, lon0) = TEST_STATION;
        let s = Strike {
            time_s: self.next_s,
            position: Position::new(
                lat0 + STRIKE_SPAN_DEG.0 * (2.0 * self.rng.uniform() - 1.0),
                lon0 + STRIKE_SPAN_DEG.1 * (2.0 * self.rng.uniform() - 1.0),
            ),
            amp_100km: self.rng.range(AMP_100KM.0, AMP_100KM.1),
            phase: TAU * self.rng.uniform(),
        };
        self.next_s += (self.rng.exponential() / self.per_s).max(MIN_GAP_S);
        Some(s)
    }
}

/// The test signal at one receiving station (module documentation).
#[derive(Clone, Debug)]
pub struct TdoaSignal {
    rate: f64,
    true_rate: f64,
    start_s: f64,
    receiver: Position,
    prop: Propagation,
    hz: f64,
    strikes: Strikes,
    planned_to_s: f64,
    waiting: Option<Strike>,
    sounding: VecDeque<Heard>,
    heard: Vec<Heard>,
    msk: Option<MskSignal>,
    n: u64,
}

impl TdoaSignal {
    /// The signal at `rate` heard at `receiver`, with `per_s` strikes a second on average from `first_s` true seconds on,
    /// the strikes drawn from `seed`. Sample 0 lies at true time 0 and the sample clock is exact until
    /// [`at_start`](Self::at_start) and [`with_clock_ppm`](Self::with_clock_ppm) change them.
    pub fn new(rate: f64, receiver: Position, per_s: f64, first_s: f64, seed: u64) -> Self {
        TdoaSignal {
            rate,
            true_rate: rate,
            start_s: 0.0,
            receiver,
            prop: Propagation::default(),
            hz: stroke_hz(rate),
            strikes: Strikes::new(seed, per_s, first_s),
            planned_to_s: f64::NEG_INFINITY,
            waiting: None,
            sounding: VecDeque::new(),
            heard: Vec::new(),
            msk: None,
            n: 0,
        }
    }

    /// Sample 0 lies at true time `start_s`. Call before [`with_timing`](Self::with_timing).
    #[must_use]
    pub fn at_start(mut self, start_s: f64) -> Self {
        self.start_s = start_s;
        self
    }

    /// The sample clock runs `ppm` parts per million fast. Call before [`with_timing`](Self::with_timing).
    #[must_use]
    pub fn with_clock_ppm(mut self, ppm: f64) -> Self {
        self.true_rate = self.rate * (1.0 + ppm * 1e-6);
        self
    }

    /// Adds the MSK timing transmitter at `position`, `offset_hz` from the tuned centre, at `baud` and `amplitude`. Its symbol
    /// boundaries leave the transmitter at whole multiples of the symbol period on the true time scale and reach the station
    /// after the ground-wave delay. Errors as [`MskSignal::new`].
    ///
    /// # Errors
    ///
    /// [`sdrkit_algo_modem::ModemError`] of the refused value.
    pub fn with_timing(
        mut self,
        position: Position,
        offset_hz: f64,
        baud: f64,
        amplitude: f64,
        seed: u64,
    ) -> Result<Self, sdrkit_algo_modem::ModemError> {
        let ppm = (self.true_rate / self.rate - 1.0) * 1e6;
        let delay = self.prop.delay_s(&position, &self.receiver);
        let period = 1.0 / baud;
        let first = (delay - self.start_s).rem_euclid(period);
        self.msk = Some(
            MskSignal::new(self.rate, offset_hz, baud, amplitude, seed)?
                .with_clock_ppm(ppm)?
                .with_first_boundary(first)?,
        );
        Ok(self)
    }

    /// The true time of input sample `n` (fractional), seconds from the epoch.
    pub fn true_time(&self, n: f64) -> f64 {
        self.start_s + n / self.true_rate
    }

    /// The strikes heard so far, in time order, with their onsets and amplitudes at the station.
    pub fn heard(&self) -> &[Heard] {
        &self.heard
    }

    /// The receiving station.
    pub fn receiver(&self) -> Position {
        self.receiver
    }

    /// Plans the strikes up to the true time `until_s`.
    fn plan(&mut self, until_s: f64) {
        while self.planned_to_s < until_s {
            let Some(s) = self.waiting.take().or_else(|| self.strikes.next()) else {
                self.planned_to_s = f64::INFINITY;
                return;
            };
            if s.time_s > until_s {
                self.waiting = Some(s);
                self.planned_to_s = until_s;
                return;
            }
            let d = distance_m(&s.position, &self.receiver).max(1e3);
            let arrival = s.time_s + self.prop.travel_time_s(d);
            let h = Heard {
                strike: s,
                onset: (arrival - self.start_s) * self.true_rate,
                amplitude: (s.amp_100km * 100e3 / d).min(AMP_MAX),
            };
            self.heard.push(h);
            self.sounding.push_back(h);
        }
    }

    /// Adds the signal of the next `out.len()` samples to `out`.
    pub fn add(&mut self, out: &mut [Complex32]) {
        let first = self.n;
        let end = first + out.len() as u64;
        self.plan(self.true_time(end as f64));
        for h in &self.sounding {
            add_stroke(
                out,
                first,
                self.true_rate,
                h.onset,
                h.amplitude,
                self.hz,
                h.strike.phase,
            );
        }
        let len = stroke_s() * self.true_rate;
        self.sounding.retain(|h| h.onset + len > end as f64);
        if let Some(m) = &mut self.msk {
            m.add(first, out);
        }
        self.n = end;
    }
}

/// The generator of the test signal.
pub struct TdoaGenerator {
    signal: TdoaSignal,
}

impl TdoaGenerator {
    /// From the stack's settings: the receiving station is the station of `cfg` when it has a position, else the
    /// `gen_station_*` settings.
    pub fn from_config(cfg: &GenConfig) -> Self {
        let s = &cfg.settings;
        let rate = cfg.rate_hz;
        let receiver = cfg.station.position.unwrap_or_else(|| {
            Position::new(s.get(&GEN_STATION_LAT), s.get(&GEN_STATION_LON))
        });
        let signal = TdoaSignal::new(
            rate,
            receiver,
            s.get(&GEN_STRIKE_RATE),
            FIRST_STRIKE_S,
            cfg.seed,
        );
        let signal = if s.get(&GEN_TIMING_TX) {
            let limit = rate / 2.5;
            let offset = s.get(&GEN_TIMING_OFFSET_HZ).clamp(-limit, limit);
            let tx = Position::new(TIMING_TX.0, TIMING_TX.1);
            match signal
                .clone()
                .with_timing(tx, offset, TIMING_BAUD, TIMING_AMPLITUDE, cfg.seed)
            {
                Ok(s) => s,
                Err(_) => signal,
            }
        } else {
            signal
        };
        TdoaGenerator { signal }
    }

    /// The signal.
    pub fn signal(&self) -> &TdoaSignal {
        &self.signal
    }
}

impl Generator for TdoaGenerator {
    fn rate_hz(&self) -> Hz {
        self.signal.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        out.fill(Complex32::default());
        self.signal.add(out);
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        Some(SignalInfo {
            power: DECLARED_POWER,
            ..SignalInfo::new(self.signal.rate)
        })
    }
    fn station_offset_hz(&self, _tune_offset_hz: Hz) -> Hz {
        0.0
    }
}

/// The generator.
///
/// # Errors
///
/// None: every setting gives a signal.
pub fn make_generator(cfg: &GenConfig) -> Result<Box<dyn Generator>, ConfigError> {
    Ok(Box::new(TdoaGenerator::from_config(cfg)))
}
