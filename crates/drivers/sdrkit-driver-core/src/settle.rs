//! Where a retune takes effect in the stream of a radio: the first sample that the radio captured after it had settled
//! on the new frequency.

use std::time::{Duration, Instant};

/// What a driver knows about a retune of its radio.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settle {
    /// The time from the end of the configure call until the radio is on the new frequency, plus the least time from the
    /// capture of a sample until the stream offers it that the times of the writes do not show.
    pub after: Duration,
    /// The most samples that can be on their way from the converter to the stream. No more than these, and those of
    /// `after`, are taken as captured before the radio settled.
    pub in_flight: u64,
}

impl Settle {
    /// The settle of a radio that says nothing: 5 ms, and half a second of samples at `rate_hz` in flight.
    pub fn of_rate(rate_hz: f64) -> Settle {
        Settle {
            after: Duration::from_millis(5),
            in_flight: (rate_hz.max(0.0) * 0.5) as u64,
        }
    }
}

/// How fast the delay estimate of [`SettleGate`] may rise, in seconds per second: the drift of the clocks.
const DELAY_RISE: f64 = 1e-3;
/// A delay estimate that no write has renewed for this long, seconds, says nothing.
const DELAY_AGE_S: f64 = 30.0;

/// A retune that has ended and whose first sample is not found yet.
#[derive(Clone, Copy, Debug)]
struct Marked {
    seq: u64,
    at: Instant,
    settle: Settle,
}

/// Finds the first sample after a retune. It maps each sample of the stream to the time of its capture: position / rate
/// plus the least delay seen between that time and the arrival of a write (the delay may rise by 1 ms per second). A
/// write that waited on its way for a time that the stream does not know ([`SettleGate::write`] with `direct` false)
/// says nothing about the delay. The first sample of a retune that ended at T is the first one captured at
/// T + [`Settle::after`] or later, and at most [`Settle::in_flight`] samples and the samples of [`Settle::after`] after
/// the first write that follows T. Without a delay, it is that last one.
#[derive(Clone, Debug)]
pub struct SettleGate {
    epoch: Instant,
    rate: f64,
    pos: u64,
    delay: Option<f64>,
    last: f64,
    marked: Option<Marked>,
    until: Option<(u64, u64)>,
    /// The stream started again: a retune that waits has its first sample at the next write.
    flush: bool,
}

impl SettleGate {
    /// A gate for a stream of `rate_hz` samples per second.
    pub fn new(rate_hz: f64) -> SettleGate {
        SettleGate {
            epoch: Instant::now(),
            rate: rate_hz.max(1.0),
            pos: 0,
            delay: None,
            last: 0.0,
            marked: None,
            until: None,
            flush: false,
        }
    }

    /// The stream started again, or its rate changed to `rate_hz`: the times of the writes before say nothing now.
    pub fn restart(&mut self, rate_hz: f64) {
        self.rate = rate_hz.max(1.0);
        self.pos = 0;
        self.delay = None;
        self.flush = self.marked.is_some() || self.until.is_some();
    }

    /// Retune number `seq` ended at `at`.
    pub fn mark(&mut self, seq: u64, at: Instant, settle: Settle) {
        self.until = None;
        self.marked = Some(Marked { seq, at, settle });
    }

    /// Retune number `seq` failed. Returns true if no retune waits for its first sample: the stream is as it was. A
    /// retune that waits takes the number `seq`.
    pub fn failed(&mut self, seq: u64) -> bool {
        match (&mut self.marked, &mut self.until) {
            (Some(m), _) => m.seq = seq,
            (None, Some(u)) => u.0 = seq,
            (None, None) => return true,
        }
        false
    }

    /// `n` samples that the stream does not write pass: lost ones, or ones left out on purpose.
    pub fn pass(&mut self, n: u64) {
        self.pos += n;
    }

    /// A write of `n` samples that arrived at `at` follows. `direct`: the radio sent them when it had captured the
    /// last of them, so their arrival tells the delay. Returns the number of a retune whose first sample is in this
    /// write or before it, and the count of samples of the write before that sample.
    pub fn write(&mut self, at: Instant, n: u64, direct: bool) -> Option<(u64, u64)> {
        let t = at.saturating_duration_since(self.epoch).as_secs_f64();
        let end = self.pos + n;
        if direct {
            let seen = t - end as f64 / self.rate;
            let delay = match self.delay {
                None => seen,
                Some(d) => (d + DELAY_RISE * (t - self.last)).min(seen),
            };
            (self.delay, self.last) = (Some(delay), t);
        }
        if let Some(m) = self.marked.take() {
            // The samples in flight, then the settle time: later ones were captured at the new frequency.
            let most =
                self.pos + m.settle.in_flight + (m.settle.after.as_secs_f64() * self.rate) as u64;
            let settled = (m.at + m.settle.after)
                .saturating_duration_since(self.epoch)
                .as_secs_f64();
            let p = match self.delay.filter(|_| t - self.last <= DELAY_AGE_S) {
                Some(delay) => ((settled - delay) * self.rate - 1e-6).ceil().max(0.0) as u64,
                None => most,
            };
            self.until = Some((m.seq, p.min(most)));
        }
        let start = std::mem::replace(&mut self.pos, end);
        let (seq, p) = self.until?;
        if std::mem::take(&mut self.flush) {
            self.until = None;
            return Some((seq, 0));
        }
        if p <= end {
            self.until = None;
            return Some((seq, p.saturating_sub(start)));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 1e6;

    fn settle(ms: u64, in_flight: u64) -> Settle {
        Settle {
            after: Duration::from_millis(ms),
            in_flight,
        }
    }

    /// Writes of 10 000 samples that arrive `delay_ms` after the capture of their last sample, up to `to` samples.
    fn run(g: &mut SettleGate, delay_ms: u64, to: u64) -> Option<(u64, u64)> {
        let mut found = None;
        while g.pos < to {
            let end = g.pos + 10_000;
            let at = g.epoch
                + Duration::from_secs_f64(end as f64 / RATE)
                + Duration::from_millis(delay_ms);
            let start = g.pos;
            if let Some((seq, k)) = g.write(at, 10_000, true) {
                found = Some((seq, start + k));
            }
        }
        found
    }

    #[test]
    fn the_first_sample_of_a_retune_is_the_first_one_captured_after_the_settle_time() {
        let mut g = SettleGate::new(RATE);
        // The writes arrive 30 ms after the capture: 30 000 samples are in flight.
        assert_eq!(run(&mut g, 30, 1_000_000), None);
        // A retune ends at 1.030 s on the clock of the arrivals: the sample captured then is number 1 000 000. With
        // 5 ms to settle, the first sample is number 1 005 000.
        let at = g.epoch + Duration::from_millis(1030);
        g.mark(7, at, settle(5, 500_000));
        assert_eq!(run(&mut g, 30, 1_100_000), Some((7, 1_005_000)));
        // It is told once.
        assert_eq!(run(&mut g, 30, 1_200_000), None);
    }

    #[test]
    fn no_more_than_the_samples_in_flight_are_taken_as_stale() {
        let mut g = SettleGate::new(RATE);
        run(&mut g, 30, 1_000_000);
        // The link fell behind since: the gate cannot know. The driver says that 20 000 samples fit in flight; the
        // 5 000 of the settle time follow them.
        g.mark(1, g.epoch + Duration::from_millis(1900), settle(5, 20_000));
        assert_eq!(run(&mut g, 30, 1_100_000), Some((1, 1_025_000)));
    }

    #[test]
    fn a_later_retune_takes_the_place_of_one_that_waits_and_a_restart_ends_it() {
        let mut g = SettleGate::new(RATE);
        run(&mut g, 0, 100_000);
        g.mark(1, g.epoch + Duration::from_millis(150), settle(0, 500_000));
        assert_eq!(run(&mut g, 0, 120_000), None);
        g.mark(2, g.epoch + Duration::from_millis(160), settle(0, 500_000));
        assert_eq!(run(&mut g, 0, 200_000), Some((2, 160_000)));
        // A retune that fails with none waiting changes nothing; with one waiting it takes its number.
        assert!(g.failed(3));
        g.mark(4, g.epoch + Duration::from_millis(900), settle(0, 500_000));
        assert!(!g.failed(5));
        // The stream starts again: the first write is the first sample.
        g.restart(RATE);
        assert_eq!(g.write(Instant::now(), 10, true), Some((5, 0)));
    }

    #[test]
    fn writes_that_waited_on_their_way_tell_no_delay() {
        let mut g = SettleGate::new(RATE);
        // No write came straight from the radio: the first sample is the one after all that fits in flight.
        let at = g.epoch + Duration::from_millis(500);
        assert_eq!(g.write(at, 100_000, false), None);
        g.mark(1, at, settle(5, 40_000));
        assert_eq!(g.write(at, 100_000, false), Some((1, 45_000)));
        // One write straight from the radio, 10 ms after its last sample; then the link falls behind by a second.
        let at = g.epoch + Duration::from_millis(310);
        assert_eq!(g.write(at, 100_000, true), None);
        let late = g.epoch + Duration::from_millis(1400);
        assert_eq!(g.write(late, 100_000, false), None);
        // A retune ends at 1.5 s. The delay of the direct write still counts: the sample captured 5 ms later is number
        // 1 495 000, which the stream reaches many writes later.
        g.mark(
            2,
            g.epoch + Duration::from_millis(1500),
            settle(5, 2_000_000),
        );
        let mut found = None;
        while found.is_none() {
            let start = g.pos;
            found = g
                .write(late, 100_000, false)
                .map(|(seq, k)| (seq, start + k));
        }
        assert_eq!(found, Some((2, 1_495_000)));
    }
}
