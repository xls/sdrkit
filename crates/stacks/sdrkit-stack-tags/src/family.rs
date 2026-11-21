//! The parts that the stacks of the family share: the front end that mixes out the offset of the dial and decimates to the
//! working rate, the tally of good and bad results with the state it gives, the log of table rows and the input level.

use crate::typed::TagsState;
use sdrkit_algo_dsp::decim::FirDecimator;
use sdrkit_algo_dsp::nco::Nco;
use sdrkit_core::publish::Published;
use sdrkit_core::{Complex32, LockState};

/// The report period of every stack of the family, seconds of signal.
pub const REPORT_S: f64 = 0.25;

/// Mixes the signal from `offset_hz` to 0 Hz, then decimates by `max(1, floor(rate / target))` with a pass band to `pass_hz`.
#[derive(Clone, Debug)]
pub struct Front {
    nco: Option<Nco>,
    dec: Option<FirDecimator<Complex32>>,
    rate: f64,
    mixed: Vec<Complex32>,
    out: Vec<Complex32>,
}

impl Front {
    /// A front end for input at `rate_hz` with the signal at `offset_hz` from the centre.
    pub fn new(rate_hz: f64, offset_hz: f64, target_hz: f64, pass_hz: f64) -> Front {
        let d = ((rate_hz / target_hz).floor() as usize).max(1);
        let dec = (d > 1)
            .then(|| FirDecimator::design(rate_hz, d, pass_hz, pass_hz, 70.0).ok())
            .flatten();
        let d = if dec.is_some() { d } else { 1 };
        Front {
            nco: (offset_hz != 0.0).then(|| Nco::new(rate_hz, -offset_hz)),
            dec,
            rate: rate_hz / d as f64,
            mixed: Vec::new(),
            out: Vec::new(),
        }
    }

    /// The working rate, Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The working-rate samples of `x`.
    pub fn process(&mut self, x: &[Complex32]) -> &[Complex32] {
        self.out.clear();
        let src: &[Complex32] = match self.nco.as_mut() {
            Some(n) => {
                self.mixed.clear();
                self.mixed.extend_from_slice(x);
                n.mix_in_place(&mut self.mixed);
                &self.mixed
            }
            None => x,
        };
        match self.dec.as_mut() {
            Some(d) => d.process(src, &mut self.out),
            None => self.out.extend_from_slice(src),
        }
        &self.out
    }
}

/// The good and bad results of a stack and the state they give: decoding after a good result in the last 8 s, energy after
/// energy or a bad result in the last 2 s, else no signal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    /// Good results.
    pub good: u64,
    /// Bad results.
    pub bad: u64,
    last_good: Option<f64>,
    last_energy: Option<f64>,
}

impl Tally {
    /// A good result at signal time `t`.
    pub fn good(&mut self, t: f64) {
        self.good += 1;
        self.last_good = Some(t);
    }

    /// A bad result at `t`.
    pub fn bad(&mut self, t: f64) {
        self.bad += 1;
        self.last_energy = Some(t);
    }

    /// Energy without a result at `t`.
    pub fn energy(&mut self, t: f64) {
        self.last_energy = Some(t);
    }

    /// Energy or a result in the last 2 s before `now`.
    pub fn recent_energy(&self, now: f64) -> bool {
        self.last_energy.is_some_and(|t| now - t <= 2.0)
            || self.last_good.is_some_and(|t| now - t <= 2.0)
    }

    /// The state at `now`.
    pub fn state(&self, now: f64) -> TagsState {
        if self.last_good.is_some_and(|t| now - t <= 8.0) {
            TagsState::Decoding
        } else if self.last_energy.is_some_and(|t| now - t <= 2.0) {
            TagsState::Energy
        } else {
            TagsState::NoSignal
        }
    }

    /// The lock state of the status at `now`.
    pub fn lock(&self, now: f64) -> LockState {
        match self.state(now) {
            TagsState::Decoding => LockState::Locked,
            TagsState::Energy => LockState::Searching,
            TagsState::NoSignal => LockState::Off,
        }
    }
}

/// The rows of a table waiting to be published, keyed by a sequence number. A row written again with its key replaces it.
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
    /// Adds a row and returns its key.
    pub fn push(&mut self, row: R) -> u64 {
        self.serial += 1;
        self.pending.push((self.serial, row));
        self.serial
    }

    /// Writes row `key` again with new content.
    pub fn update(&mut self, key: u64, row: R) {
        self.pending.retain(|(k, _)| *k != key);
        self.pending.push((key, row));
    }

    /// Empties the table at the next publish.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.cleared = true;
    }

    /// Writes the waiting rows: `clear` empties the table first when it was cleared, `put` writes one row with its key.
    pub fn publish(
        &mut self,
        p: &mut Published,
        clear: impl FnOnce(&mut Published),
        mut put: impl FnMut(&mut Published, &str, &R),
    ) {
        if std::mem::take(&mut self.cleared) {
            clear(p);
        }
        let mut key = String::with_capacity(8);
        for (k, r) in self.pending.drain(..) {
            key.clear();
            key.push_str(&format!("{k:06}"));
            put(p, &key, &r);
        }
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
            self.last = Some(sdrkit_algo_dsp::meter::power_to_db(self.sum / self.n as f64));
            self.sum = 0.0;
            self.n = 0;
        }
        self.last
    }
}


/// `20 log10 x`, with a floor of -600 dB.
pub fn db20(x: f64) -> f64 {
    20.0 * x.max(1e-30).log10()
}

/// Bytes as upper-case hex pairs separated by spaces.
pub fn hex_bytes(b: &[u8]) -> String {
    let mut s = String::with_capacity(3 * b.len());
    for (i, x) in b.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{x:02X}"));
    }
    s
}
