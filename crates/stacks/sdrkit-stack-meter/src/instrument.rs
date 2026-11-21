//! The parts that the meter stacks share: the front end to a working rate, the one-sided spectrum of a stretch of a real
//! signal with its band sums, the change rule of a state that must hold for two measurements, the field reader of the
//! verify checks, and for the spectrum watchers the tally of readings, the log of rows and the input level.

use crate::typed::WatchState;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::spectrum::{Scaling, Welch};
use sdrkit_algo_dsp::window::Window;
use sdrkit_core::interface::{Fields, Table};
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, LockState, Results, Value};

/// Decimates the input by `max(1, floor(rate / top))` to the working rate and drops the first stretch while the filter fills.
#[derive(Clone, Debug)]
pub struct Front {
    dec: Option<FirDecimator<Complex32>>,
    rate: f64,
    skip: usize,
    out: Vec<Complex32>,
}

impl Front {
    /// A front end for input at `rate_hz` that keeps the band to `pass_hz` free of aliases, runs at `top_hz` to twice that,
    /// and drops the first `skip_s` of its output.
    pub fn new(rate_hz: f64, top_hz: f64, pass_hz: f64, skip_s: f64) -> Front {
        let d = ((rate_hz / top_hz).floor() as usize).max(1);
        let dec = (d > 1)
            .then(|| FirDecimator::design(rate_hz, d, pass_hz, pass_hz, 70.0).ok())
            .flatten();
        let d = if dec.is_some() { d } else { 1 };
        let rate = rate_hz / d as f64;
        Front {
            dec,
            rate,
            skip: (skip_s * rate).round() as usize,
            out: Vec::new(),
        }
    }

    /// The working rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The working-rate samples of `x`, without the first stretch.
    pub fn process(&mut self, x: &[Complex32]) -> &[Complex32] {
        self.out.clear();
        match self.dec.as_mut() {
            Some(d) => d.process(x, &mut self.out),
            None => self.out.extend_from_slice(x),
        }
        let drop = self.skip.min(self.out.len());
        self.skip -= drop;
        &self.out[drop..]
    }
}

/// The one-sided Welch spectrum of a real signal: Hann window, frames overlapping by half, each bin the mean squared
/// amplitude, so a sine of amplitude `A` sums to `A^2 / 2` over its bins.
#[derive(Debug)]
pub struct Spectrum {
    welch: Welch,
    two: Vec<f32>,
    one: Vec<f64>,
    bin_hz: f64,
    ready: bool,
}

impl Spectrum {
    /// A spectrum at `rate_hz` with frames of the largest power of two not above `rate_hz / per_hz` (bins of `per_hz` to
    /// twice that).
    pub fn new(rate_hz: f64, per_hz: f64) -> Spectrum {
        let want = (rate_hz / per_hz).max(16.0) as usize;
        let n = 1usize << (usize::BITS - 1 - want.leading_zeros());
        let welch = Welch::new(n, Window::Hann, 0.5, Scaling::Power).expect("a power of two from 16 up");
        Spectrum {
            welch,
            two: vec![0.0; n],
            one: vec![0.0; n / 2],
            bin_hz: rate_hz / n as f64,
            ready: false,
        }
    }

    /// The width of a bin, Hz.
    pub fn bin_hz(&self) -> f64 {
        self.bin_hz
    }

    /// The one-sided bins from 0 Hz.
    pub fn bins(&self) -> &[f64] {
        &self.one
    }

    /// Computes the spectrum of `x`. False when `x` is shorter than a frame (the spectrum is then empty).
    pub fn compute(&mut self, x: &[f32]) -> bool {
        self.ready = self.welch.compute_real(x, &mut self.two).is_ok();
        let n = self.two.len();
        let c = n / 2;
        for (k, b) in self.one.iter_mut().enumerate() {
            *b = if !self.ready {
                0.0
            } else if k == 0 {
                f64::from(self.two[c])
            } else {
                f64::from(self.two[c + k]) + f64::from(self.two[c - k])
            };
        }
        self.ready
    }

    /// The sum of the bins from `floor(f0 / bin)` to `ceil(f1 / bin)`.
    pub fn band(&self, f0: f64, f1: f64) -> f64 {
        let last = self.one.len().saturating_sub(1);
        let a = ((f0 / self.bin_hz).floor().max(0.0) as usize).min(last);
        let b = ((f1 / self.bin_hz).ceil().max(0.0) as usize).min(last);
        if b < a {
            return 0.0;
        }
        self.one[a..=b].iter().sum()
    }

    /// The peak of the sine with the power of `band(f0, f1)`: `sqrt(2 band)`.
    pub fn amplitude(&self, f0: f64, f1: f64) -> f64 {
        (2.0 * self.band(f0, f1)).sqrt()
    }
}

/// A yes-or-no state that takes a new value only when two measurements in a row give it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    state: Option<bool>,
    pending: Option<bool>,
}

impl Held {
    /// The state; `None` before the first measurement.
    pub fn get(&self) -> Option<bool> {
        self.state
    }

    /// Takes a measurement. Returns the new state when it changed: the first measurement sets it at once, a later change must
    /// be seen twice in a row.
    pub fn push(&mut self, now: bool) -> Option<bool> {
        match self.state {
            None => {
                self.state = Some(now);
                self.pending = None;
                Some(now)
            }
            Some(s) if s == now => {
                self.pending = None;
                None
            }
            Some(_) if self.pending == Some(now) => {
                self.state = Some(now);
                self.pending = None;
                Some(now)
            }
            Some(_) => {
                self.pending = Some(now);
                None
            }
        }
    }
}

/// The value of the result field `key` of `r` as a number (a yes is 1); 0 when it has none.
pub fn number(r: &Results, key: &str) -> f64 {
    let Some(v) = r.decl().field_id(key).and_then(|id| r.fields().value(id)) else {
        return 0.0;
    };
    match v {
        Value::I64(x) => *x as f64,
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        Value::Bool(b) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

/// A block of power under this, dB, holds digital silence (no samples from the radio): it gives no floor reading.
pub const SILENCE_DB: f64 = -250.0;

/// The good and bad readings of a watcher and the state that they give: measuring after a good reading in the last 8 s,
/// energy after energy or a bad reading in the last 2 s, else nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    /// Good readings.
    pub good: u64,
    /// Bad readings.
    pub bad: u64,
    last_good: Option<f64>,
    last_energy: Option<f64>,
}

impl Tally {
    /// A good reading at signal time `t`.
    pub fn good(&mut self, t: f64) {
        self.good += 1;
        self.last_good = Some(t);
    }

    /// A bad reading at `t`.
    pub fn bad(&mut self, t: f64) {
        self.bad += 1;
        self.last_energy = Some(t);
    }

    /// Energy without a reading at `t`.
    pub fn energy(&mut self, t: f64) {
        self.last_energy = Some(t);
    }

    /// The state at `now`.
    pub fn state(&self, now: f64) -> WatchState {
        if self.last_good.is_some_and(|t| now - t <= 8.0) {
            WatchState::Measuring
        } else if self.last_energy.is_some_and(|t| now - t <= 2.0) {
            WatchState::Energy
        } else {
            WatchState::Nothing
        }
    }

    /// The lock state of the status at `now`.
    pub fn lock(&self, now: f64) -> LockState {
        match self.state(now) {
            WatchState::Measuring => LockState::Locked,
            WatchState::Energy => LockState::Searching,
            WatchState::Nothing => LockState::Off,
        }
    }
}

/// The rows of a log waiting to be published, keyed by a sequence number.
#[derive(Clone, Debug)]
pub struct Log<R> {
    pending: Vec<(u64, R)>,
    serial: u64,
    cleared: bool,
}

impl<R> Default for Log<R> {
    fn default() -> Self {
        Log {
            pending: Vec::new(),
            serial: 0,
            cleared: false,
        }
    }
}

impl<R> Log<R> {
    /// Adds a row.
    pub fn push(&mut self, row: R) {
        self.serial += 1;
        self.pending.push((self.serial, row));
    }

    /// The rows written so far.
    pub fn count(&self) -> u64 {
        self.serial
    }

    /// Empties the table at the next publish.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.cleared = true;
    }

    /// Writes the waiting rows into `t`.
    pub fn publish<S: Fields + 'static>(&mut self, t: &Table<S>, p: &mut Published)
    where
        R: Fields<Static = S>,
    {
        if std::mem::take(&mut self.cleared) {
            p.clear_rows(t);
        }
        for (k, r) in self.pending.drain(..) {
            t.put(p, &format!("{k:06}"), &r);
        }
    }
}

/// Writes `rows` as the whole of the list `t`, keyed by place.
pub fn publish_list<S: Fields + 'static, R: Fields<Static = S>>(
    t: &Table<S>,
    rows: &[R],
    p: &mut Published,
) {
    p.clear_rows(t);
    for (k, r) in rows.iter().enumerate() {
        t.put(p, &format!("{k:03}"), r);
    }
}

/// The mean power of the input since the last reading, dBFS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InputLevel {
    sum: f64,
    n: u64,
    last: Option<f64>,
}

impl InputLevel {
    /// Takes input samples.
    pub fn push(&mut self, x: &[Complex32]) {
        self.sum += x.iter().map(|z| f64::from(z.norm_sqr())).sum::<f64>();
        self.n += x.len() as u64;
    }

    /// The level since the last call, or the last level when no sample came.
    pub fn read(&mut self) -> Option<f64> {
        if self.n > 0 {
            self.last = Some(10.0 * (self.sum / self.n as f64).max(1e-30).log10());
            self.sum = 0.0;
            self.n = 0;
        }
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    #[test]
    fn a_sine_sums_to_half_its_square_and_the_state_needs_two_measurements() {
        let rate = 48e3;
        let x: Vec<f32> = (0..48_000)
            .map(|i| (3.0 * (TAU * 1000.0 * i as f64 / rate).sin()) as f32)
            .collect();
        let mut s = Spectrum::new(rate, 5.0);
        assert!(s.compute(&x));
        assert!((s.band(950.0, 1050.0) - 4.5).abs() < 0.05, "{}", s.band(950.0, 1050.0));
        assert!((s.amplitude(950.0, 1050.0) - 3.0).abs() < 0.02);
        let mut h = Held::default();
        assert_eq!(h.push(true), Some(true));
        assert_eq!(h.push(false), None);
        assert_eq!(h.push(true), None);
        assert_eq!(h.push(false), None);
        assert_eq!(h.push(false), Some(false));
    }
}
