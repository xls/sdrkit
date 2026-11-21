//! Keying detection for Morse and other on-off keyed carriers: [`KeyState`] turns key-down decisions into the lengths of marks
//! and spaces, and [`KeyDetector`] makes those decisions from an envelope with levels and timing that scale with the dot length.
//!
//! ```
//! use sdrkit_algo_modem::key::KeyDetector;
//! let mut k = KeyDetector::new(1000.0).unwrap();
//! let mut marks = Vec::new();
//! // 1 s of noise level 0.1, then a 300 ms mark of level 1, then 1 s of noise.
//! for i in 0..2300 {
//!     let env = if (1000..1300).contains(&i) { 1.0 } else { 0.1 };
//!     if let Some(e) = k.step(env, 0.1) {
//!         if e.on { marks.push(e.seconds) }
//!     }
//! }
//! assert_eq!(marks.len(), 1);
//! assert!((marks[0] - 0.3).abs() < 0.02);
//! ```

use crate::{ModemError, Param};

/// One mark or space that ended, or a space still going.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyEvent {
    /// True for a mark (key down).
    pub on: bool,
    /// The length, seconds.
    pub seconds: f64,
    /// False for a space still going, reported from time to time so that a receiver can end a word.
    pub closed: bool,
    /// The mean level the caller provided during the element.
    pub level: f32,
    /// The time the element ended (or the time of the report), seconds, in the caller's time.
    pub end: f64,
}

/// Turns key-down decisions, each covering `n` samples, into [`KeyEvent`]s. A change must last `hold` decisions (a debounce).
/// The event is placed where the change began.
#[derive(Clone, Debug)]
pub struct KeyState {
    rate: f64,
    on: bool,
    pending: u32,
    hold: u32,
    len: u64,
    level_sum: f64,
    level_n: u64,
    open_every: u64,
    open_reported: u64,
}

impl Default for KeyState {
    /// Decisions on samples at 1 kHz, no debounce, no reports of a space still going.
    fn default() -> Self {
        KeyState {
            rate: 1000.0,
            on: false,
            pending: 0,
            hold: 1,
            len: 0,
            level_sum: 0.0,
            level_n: 0,
            open_every: 0,
            open_reported: 0,
        }
    }
}

impl KeyState {
    /// For decisions on samples at `rate` Hz. A change must last `hold` decisions (at least 1). While the key is up, a space
    /// still going is reported every `open_every_s` seconds (0: never). Errors: a rate that is not a positive number, or a
    /// negative report time.
    pub fn new(rate: f64, hold: u32, open_every_s: f64) -> Result<Self, ModemError> {
        if !(rate.is_finite() && rate > 0.0) {
            return Param::Rate.positive(rate);
        }
        if !(open_every_s.is_finite() && open_every_s >= 0.0) {
            return Param::ReportTime.at_least(0.0, open_every_s);
        }
        Ok(KeyState {
            rate,
            on: false,
            pending: 0,
            hold: hold.max(1),
            len: 0,
            level_sum: 0.0,
            level_n: 0,
            open_every: (rate * open_every_s).round() as u64,
            open_reported: 0,
        })
    }

    /// True while the key is down.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The time in the present state, seconds.
    pub fn seconds_in_state(&self) -> f64 {
        self.len as f64 / self.rate
    }

    /// Sets the number of decisions a change must last (at least 1).
    pub fn set_hold(&mut self, hold: u32) {
        self.hold = hold.max(1);
    }

    /// Forgets the state: the key is up and no time has passed.
    pub fn reset(&mut self) {
        self.on = false;
        self.pending = 0;
        self.len = 0;
        self.level_sum = 0.0;
        self.level_n = 0;
        self.open_reported = 0;
    }

    /// One decision `want` (key down) with the `level` seen, covering `n` samples that end at `time` seconds. Returns the
    /// element that ended, or a report of a space still going.
    pub fn decide(&mut self, want: bool, level: f32, n: u64, time: f64) -> Option<KeyEvent> {
        if want != self.on {
            self.pending += 1;
        } else {
            self.pending = 0;
        }
        self.len += n;
        self.level_sum += level as f64 * n as f64;
        self.level_n += n;
        if self.pending >= self.hold {
            // The change began `pending` decisions ago.
            let back = (self.pending as u64 * n).min(self.len);
            let e = KeyEvent {
                on: self.on,
                seconds: (self.len - back) as f64 / self.rate,
                closed: true,
                level: (self.level_sum / self.level_n.max(1) as f64) as f32,
                end: time - back as f64 / self.rate,
            };
            self.on = want;
            self.len = back;
            self.level_sum = 0.0;
            self.level_n = 0;
            self.pending = 0;
            self.open_reported = 0;
            return Some(e);
        }
        if !self.on && self.open_every > 0 && self.len / self.open_every > self.open_reported {
            self.open_reported = self.len / self.open_every;
            return Some(KeyEvent {
                on: false,
                seconds: self.len as f64 / self.rate,
                closed: false,
                level: 0.0,
                end: time,
            });
        }
        None
    }
}

/// The longest smoothing window, seconds.
const SMOOTH_MAX_S: f64 = 0.040;

/// The key detector of a Morse receiver: it follows the mark and noise levels of an envelope and decides key down.
///
/// The envelope is smoothed over 0.15 dot lengths (2 to 20 ms, twice that on weak signals). A Schmitt trigger switches at
/// 0.55 (up) and 0.42 (down) of the way from the noise level to the mark level. The key stays up until the mark level is 2.6
/// times the noise level, so noise alone returns no marks. A change must last 0.2 dot lengths (3 to 15 ms). It allocates only
/// in [`KeyDetector::new`].
#[derive(Clone, Debug)]
pub struct KeyDetector {
    rate: f64,
    ring: Vec<f32>,
    pos: usize,
    sum: f32,
    len: usize,
    mark: f32,
    noise: f32,
    started: bool,
    n: u64,
    samples: u64,
    state: KeyState,
    /// Per-sample rates of the level followers: fast rise, slow fall, gated rise.
    a_fast: f32,
    a_slow: f32,
    a_gate: f32,
}

impl KeyDetector {
    /// A detector for an envelope at `rate` Hz (200 Hz to 1 MHz). Errors for a rate outside that range.
    pub fn new(rate: f64) -> Result<Self, ModemError> {
        if !(200.0..=1e6).contains(&rate) {
            return Param::Rate.range(200.0, 1e6, rate);
        }
        // The rates were set for 1 kHz: keep their time constants at any rate.
        let per = |a: f64| (1.0 - (1.0 - a).powf(1000.0 / rate)) as f32;
        Ok(KeyDetector {
            rate,
            ring: vec![0.0; (SMOOTH_MAX_S * rate).ceil() as usize + 1],
            pos: 0,
            sum: 0.0,
            len: 1,
            mark: 0.0,
            noise: 0.0,
            started: false,
            n: 0,
            samples: 0,
            state: KeyState::new(rate, 1, 0.0)?,
            a_fast: per(0.3),
            a_slow: per(0.002),
            a_gate: per(0.03),
        })
    }

    /// True while the key is down.
    pub fn is_on(&self) -> bool {
        self.state.is_on()
    }

    /// The time in the present state, seconds.
    pub fn seconds_in_state(&self) -> f64 {
        self.state.seconds_in_state()
    }

    /// The mark level.
    pub fn mark_level(&self) -> f32 {
        self.mark
    }

    /// The noise level.
    pub fn noise_level(&self) -> f32 {
        self.noise
    }

    /// The mark level over the noise level, dB.
    pub fn snr_db(&self) -> f32 {
        20.0 * (self.mark / self.noise.max(1e-12)).max(1e-6).log10()
    }

    /// Forgets the levels and the state.
    pub fn reset(&mut self) {
        self.ring.fill(0.0);
        self.pos = 0;
        self.sum = 0.0;
        self.len = 1;
        self.mark = 0.0;
        self.noise = 0.0;
        self.started = false;
        self.n = 0;
        self.samples = 0;
        self.state.reset();
    }

    /// One envelope sample. `dot_s` is the dot length the timing decoder follows, seconds. Returns the element that ended.
    /// A sample that is not finite counts as 0.
    pub fn step(&mut self, env: f32, dot_s: f64) -> Option<KeyEvent> {
        let env = if env.is_finite() { env.max(0.0) } else { 0.0 };
        let dot_s = if dot_s.is_finite() {
            dot_s.clamp(0.005, 2.0)
        } else {
            0.06
        };
        let ms = self.rate / 1000.0; // samples a millisecond
        if !self.started {
            self.started = true;
            self.noise = env;
            self.mark = env;
        }
        let cap = self.ring.len();
        let weak = if self.mark < 4.0 * self.noise {
            2.0
        } else {
            1.0
        };
        let want = ((0.15 * dot_s).clamp(0.002, 0.020) * weak * self.rate).round() as usize;
        let want = want.clamp(1, cap - 1);
        // A moving average whose length follows the dot length.
        self.sum += env - self.ring[(self.pos + cap - self.len) % cap];
        self.ring[self.pos] = env;
        self.pos = (self.pos + 1) % cap;
        if want != self.len {
            self.len = want;
            self.sum = (0..self.len)
                .map(|k| self.ring[(self.pos + cap - 1 - k) % cap])
                .sum();
        }
        let ceiling = if self.mark > 0.0 {
            3.0 * self.mark.max(self.noise * 3.0)
        } else {
            f32::MAX
        };
        let s = (self.sum / self.len as f32).min(ceiling);
        if self.state.is_on() {
            let a = if s > self.mark {
                self.a_fast
            } else {
                self.a_slow
            };
            self.mark += a * (s - self.mark);
        } else {
            let tau = (0.7 * self.rate).max(10.0 * dot_s * self.rate) as f32;
            self.n += 1;
            let tc = ((5.0 * dot_s * self.rate).max(30.0 * ms) as f32).min(self.n as f32);
            self.noise += (s - self.noise) / tc;
            if (self.n as f64) < 50.0 * ms {
                self.mark = self.noise;
            }
            if s > self.mark {
                // Only a level well clear of the noise raises the mark level quickly: noise peaks must not open the gate.
                let a = if s > 3.0 * self.noise {
                    self.a_gate
                } else {
                    self.a_slow
                };
                self.mark += a * (s - self.mark);
            } else {
                self.mark += (self.noise - self.mark) / tau;
            }
        }
        let span = self.mark - self.noise;
        let gate = self.mark >= 2.6 * self.noise;
        let raw = if self.state.is_on() {
            s > self.noise + 0.42 * span
        } else {
            gate && s > self.noise + 0.55 * span
        };
        self.state
            .set_hold(((0.2 * dot_s).clamp(0.003, 0.015) * self.rate).round() as u32);
        self.samples += 1;
        self.state
            .decide(raw, s, 1, self.samples as f64 / self.rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_state_reports_lengths() {
        let mut k = KeyState::new(1000.0, 2, 0.5).unwrap();
        let mut out = Vec::new();
        let mut t = 0.0;
        // 100 ms on, 500 ms off, in decisions of 10 samples.
        for i in 0..70 {
            t += 0.01;
            out.extend(k.decide((10..20).contains(&i), 1.0, 10, t));
        }
        let closed: Vec<_> = out.iter().filter(|k| k.closed).collect();
        assert_eq!(closed.len(), 2, "{out:?}");
        assert!(!closed[0].on && (closed[0].seconds - 0.1).abs() < 1e-9);
        assert!(closed[1].on && (closed[1].seconds - 0.1).abs() < 1e-9);
        assert!((closed[1].end - 0.2).abs() < 1e-9, "{}", closed[1].end);
        // The space after the mark is reported while it goes on.
        assert!(
            out.iter()
                .any(|k| !k.closed && (k.seconds - 0.5).abs() < 1e-9),
            "{out:?}"
        );
        assert!(KeyState::new(0.0, 1, 0.0).is_err());
    }

    /// A keyed envelope over noise: the marks come out with their lengths, and noise alone returns none.
    #[test]
    fn detector_reads_keyed_envelope() {
        for rate in [1000.0, 4000.0] {
            let mut k = KeyDetector::new(rate).unwrap();
            let mut x = 0x1234_5678u32;
            let mut noise = || {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                0.1 + 0.05 * (x as f32 / u32::MAX as f32)
            };
            // 1 s of noise, then marks of 60, 180, 60 ms with 60 ms spaces, then 1 s of noise.
            let marks = [(1.00, 1.06), (1.12, 1.30), (1.36, 1.42)];
            let n = (2.5 * rate) as usize;
            let mut got = Vec::new();
            for i in 0..n {
                let t = i as f64 / rate;
                let on = marks.iter().any(|m| (m.0..m.1).contains(&t));
                let env = if on { 1.0 } else { noise() };
                if let Some(e) = k.step(env, 0.06) {
                    if e.on {
                        got.push((e.end - e.seconds, e.seconds));
                    }
                }
            }
            assert_eq!(got.len(), 3, "{rate}: {got:?}");
            // The first mark starts late while the mark level rises above the gate.
            assert!(got[0].1 > 0.03, "{rate}: {got:?}");
            for (g, m) in got.iter().zip(marks).skip(1) {
                assert!((g.1 - (m.1 - m.0)).abs() < 0.013, "{rate}: {got:?}");
                assert!((g.0 - m.0).abs() < 0.013, "{rate}: {got:?}");
            }
        }
        let mut k = KeyDetector::new(1000.0).unwrap();
        let mut x = 7u32;
        for _ in 0..30_000 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let env = (x >> 8) as f32 / (1u32 << 24) as f32;
            assert!(k.step(env, 0.06).is_none_or(|e| !e.on));
        }
        assert!(KeyDetector::new(10.0).is_err());
    }
}
