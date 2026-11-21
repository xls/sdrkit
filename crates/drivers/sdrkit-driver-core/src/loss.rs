//! The detection of samples that the radio or its link lost, from the times and sizes of the writes into the ring, and the
//! meter of the delivered rate.

use sdrkit_core::Hz;
use std::collections::VecDeque;

/// The hold after a start or a reconnect, in seconds.
pub const HOLD_START_S: f64 = 1.0;
/// The hold after a retune, in seconds.
pub const HOLD_RETUNE_S: f64 = 0.3;
const QUARTER_S: f64 = 0.25;
const SETTLE_S: f64 = 3.0;
const SINK_PER_S: f64 = 0.0005;
const JITTER_WINDOW: usize = 40; // quarters in 10 s
const WINDOW_S: f64 = 2.0;
const MIN_WINDOW_S: f64 = 1.5;

/// A loss the detector found.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loss {
    /// When it was found (seconds, on the caller's clock).
    pub at_s: f64,
    /// How many samples.
    pub samples: u64,
}

/// Finds samples a radio did not deliver, and measures the delivered rate. Feed it every write ([`LossDetector::write`]) with a
/// monotonic time in seconds and the total samples offered so far (written plus dropped, after the write).
#[derive(Clone, Debug)]
pub struct LossDetector {
    rate: Hz,
    hold_until: Option<f64>,
    hold_len: f64,
    anchor: Option<(f64, u64)>,
    quarter_start: f64,
    quarter_min: f64,
    quarters: VecDeque<f64>,
    jitter: VecDeque<f64>,
    floor: Option<f64>,
    floor_t: f64,
    store: f64,
    last_size: usize,
    // Delivered-rate window.
    window: Option<(f64, u64)>,
    last_write: Option<(f64, u64)>,
    delivered_pct: Option<f64>,
    // Totals.
    lost: u64,
    events: u64,
    last_loss: Option<f64>,
    pending_start: bool,
}

impl LossDetector {
    /// A detector for a radio running at `rate_hz`. It holds for [`HOLD_START_S`] from the first write.
    pub fn new(rate_hz: Hz) -> Self {
        LossDetector {
            rate: rate_hz,
            hold_until: None,
            hold_len: HOLD_START_S,
            anchor: None,
            quarter_start: 0.0,
            quarter_min: f64::INFINITY,
            quarters: VecDeque::with_capacity(5),
            jitter: VecDeque::with_capacity(JITTER_WINDOW + 1),
            floor: None,
            floor_t: 0.0,
            store: 0.0,
            last_size: 0,
            window: None,
            last_write: None,
            delivered_pct: None,
            lost: 0,
            events: 0,
            last_loss: None,
            pending_start: true,
        }
    }

    fn restart(&mut self, hold_s: f64) {
        self.hold_len = hold_s;
        self.hold_until = None;
        self.pending_start = true;
        self.anchor = None;
        self.quarters.clear();
        self.jitter.clear();
        self.quarter_min = f64::INFINITY;
        self.floor = None;
        self.store = 0.0;
        self.window = None;
    }

    /// The stream started again after a start or a reconnect, at `rate_hz`: hold for 1 s, then re-anchor.
    pub fn restarted(&mut self, rate_hz: Hz) {
        self.rate = rate_hz;
        self.restart(HOLD_START_S);
    }

    /// The radio was retuned (perhaps to a new rate): hold for 0.3 s, then re-anchor.
    pub fn retuned(&mut self, rate_hz: Hz) {
        self.rate = rate_hz;
        self.restart(HOLD_RETUNE_S);
    }

    /// Counts a known gap (the link was down for that many samples) as radio-side loss.
    pub fn add_gap(&mut self, now_s: f64, samples: u64) {
        if samples > 0 {
            self.lost += samples;
            self.events += 1;
            self.last_loss = Some(now_s);
        }
    }

    /// The rate the detector counts at.
    pub fn rate(&self) -> Hz {
        self.rate
    }

    /// Total radio-side samples lost, events, and the time of the last loss.
    pub fn totals(&self) -> (u64, u64, Option<f64>) {
        (self.lost, self.events, self.last_loss)
    }

    /// The delivered rate of the last complete window, in percent.
    pub fn delivered_pct(&self) -> Option<f64> {
        self.delivered_pct
    }

    /// One write of `size` samples at `now_s`; `offered` is the running total offered after it. Returns a loss when one is found.
    pub fn write(&mut self, now_s: f64, offered: u64, size: usize) -> Option<Loss> {
        self.last_size = size;
        if self.pending_start {
            self.pending_start = false;
            self.hold_until = Some(now_s + self.hold_len);
        }
        if let Some(h) = self.hold_until {
            self.last_write = Some((now_s, offered));
            if now_s < h {
                return None;
            }
            // The first write after the hold is the anchor.
            self.hold_until = None;
            self.anchor = Some((now_s, offered));
            self.quarter_start = now_s;
            self.quarter_min = 0.0;
            self.floor_t = now_s;
            self.window = Some((now_s, offered));
            return None;
        }
        let (t0, o0) = self.anchor?;
        self.update_window(now_s, offered);
        let lag = (now_s - t0) * self.rate - (offered - o0) as f64;
        self.quarter_min = self.quarter_min.min(lag);
        if now_s - self.quarter_start < QUARTER_S {
            return None;
        }
        let qmin = self.quarter_min;
        self.quarter_start = now_s;
        self.quarter_min = f64::INFINITY;
        self.quarter(now_s, qmin, now_s - t0)
    }

    fn update_window(&mut self, now_s: f64, offered: u64) {
        if let Some((ws, wo)) = self.window {
            let dt = now_s - ws;
            if dt >= WINDOW_S {
                if dt >= MIN_WINDOW_S && self.rate > 0.0 {
                    self.delivered_pct = Some((offered - wo) as f64 / (dt * self.rate) * 100.0);
                }
                self.window = Some((now_s, offered));
            }
        }
        self.last_write = Some((now_s, offered));
    }

    fn quarter(&mut self, now_s: f64, qmin: f64, since_anchor: f64) -> Option<Loss> {
        self.quarters.push_back(qmin);
        if self.quarters.len() > 4 {
            self.quarters.pop_front();
        }
        let e = self.quarters.iter().copied().fold(f64::INFINITY, f64::min);
        self.jitter.push_back(qmin - e);
        if self.jitter.len() > JITTER_WINDOW {
            self.jitter.pop_front();
        }
        let sink = SINK_PER_S * self.rate;
        let dt = now_s - self.floor_t;
        self.floor_t = now_s;
        let Some(f) = self.floor else {
            self.floor = Some(e);
            return None;
        };
        let sunk = f - sink * dt;
        let rise = e - sunk;
        self.floor = Some(e.max(sunk));
        self.store = (self.store - 2.0 * sink * dt).max(0.0);
        if since_anchor < SETTLE_S || rise <= 0.0 {
            return None;
        }
        let threshold = self.threshold();
        let found = if rise > threshold {
            rise
        } else {
            self.store += rise;
            if self.store > threshold {
                std::mem::take(&mut self.store)
            } else {
                return None;
            }
        };
        let samples = found.round() as u64;
        self.lost += samples;
        self.events += 1;
        self.last_loss = Some(now_s);
        Some(Loss {
            at_s: now_s,
            samples,
        })
    }

    fn threshold(&self) -> f64 {
        let mut buf = [0.0f64; JITTER_WINDOW + 1];
        let n = self.jitter.len().min(buf.len());
        let j = &mut buf[..n];
        for (d, s) in j.iter_mut().zip(&self.jitter) {
            *d = *s;
        }
        j.sort_unstable_by(f64::total_cmp);
        let median = if j.is_empty() { 0.0 } else { j[j.len() / 2] };
        (0.003 * self.rate)
            .max(self.last_size as f64 / 2.0)
            .max(4.0 * median)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulates a radio at `rate` writing blocks of `block` samples every block period, plus `late` jitter pattern, `drop_at`
    /// (seconds) where one block is lost, and a clock error in ppm. Returns the detector after `secs`.
    fn run(
        rate: f64,
        block: usize,
        secs: f64,
        drop_at: Option<f64>,
        ppm: f64,
        jitter: bool,
    ) -> LossDetector {
        let mut d = LossDetector::new(rate);
        let period = block as f64 / (rate * (1.0 + ppm * 1e-6));
        let mut offered = 0u64;
        let mut t = 0.0;
        let mut i = 0u64;
        let mut dropped = false;
        while t < secs {
            t += period;
            i += 1;
            if let Some(at) = drop_at {
                if !dropped && t >= at {
                    dropped = true;
                    continue; // the transfer never arrives
                }
            }
            offered += block as u64;
            // Jitter: every 7th write is 20 ms late, the next one on time (both arrive in a burst).
            let when = if jitter && i.is_multiple_of(7) {
                t + 0.02
            } else {
                t
            };
            d.write(when, offered, block);
        }
        d
    }

    #[test]
    fn a_steady_radio_loses_nothing() {
        let d = run(2.4e6, 16384, 12.0, None, 0.0, false);
        assert_eq!(d.totals().0, 0);
        assert!((d.delivered_pct().unwrap() - 100.0).abs() < 1.0);
    }

    #[test]
    fn late_writes_are_not_losses() {
        let d = run(2.4e6, 16384, 12.0, None, 0.0, true);
        assert_eq!(d.totals().0, 0);
    }

    #[test]
    fn a_slightly_fast_or_slow_clock_is_not_a_loss() {
        assert_eq!(run(2.4e6, 16384, 15.0, None, 100.0, true).totals().0, 0);
        assert_eq!(run(2.4e6, 16384, 15.0, None, -100.0, true).totals().0, 0);
    }

    #[test]
    fn a_lost_transfer_is_counted_once_with_its_size() {
        let block = 65536;
        let d = run(2.4e6, block, 12.0, Some(6.0), 0.0, true);
        let (lost, events, at) = d.totals();
        assert_eq!(events, 1);
        assert!((lost as i64 - block as i64).abs() < 2000, "lost {lost}");
        let at = at.unwrap();
        assert!((6.0..7.6).contains(&at), "found at {at}");
    }

    #[test]
    fn nothing_counts_during_the_first_seconds() {
        let d = run(2.4e6, 65536, 3.0, Some(1.5), 0.0, false);
        assert_eq!(d.totals().0, 0);
    }

    #[test]
    fn a_slow_radio_shows_in_the_delivered_rate() {
        let d = run(2.4e6, 16384, 8.0, None, -50_000.0, false);
        let p = d.delivered_pct().unwrap();
        assert!((94.0..96.0).contains(&p), "{p}");
    }

    #[test]
    fn a_link_gap_is_radio_side_loss() {
        let mut d = LossDetector::new(1e6);
        d.add_gap(1.0, 500_000);
        assert_eq!(d.totals(), (500_000, 1, Some(1.0)));
    }
}
