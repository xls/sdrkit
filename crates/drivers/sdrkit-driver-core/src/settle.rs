//! Where a change of a radio takes effect in its stream. A retune: the first sample that the radio captured after it
//! had settled on the new frequency. A change of gain, of rate or of another setting, and the start of a stream: the
//! stretch of samples captured while the radio settled.

use std::time::{Duration, Instant};

/// What a driver knows about a change of its radio: a retune, a gain, a rate, the start of a stream.
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
/// A hold ([`SettleGate::hold`]) keeps the stretch of a change open for at most this long, seconds of samples.
const HOLD_MAX_S: f64 = 0.5;

/// A retune that has ended and whose first sample is not found yet.
#[derive(Clone, Copy, Debug)]
struct Marked {
    seq: u64,
    at: Instant,
    settle: Settle,
}

/// A change other than a retune that has ended and whose stretch is not placed yet.
#[derive(Clone, Copy, Debug)]
struct Change {
    /// `None`: the stretch begins at the next write (the start of a stream).
    began: Option<Instant>,
    ended: Instant,
    settle: Settle,
}

/// What [`SettleGate::write`] found, as counts of the samples of the write before each place.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Found {
    /// The number of a retune whose first sample is in this write or before it, and the place of that sample.
    pub moved: Option<(u64, u64)>,
    /// The first sample of the stretch of a change. It can lie before this write (below 0) or after it.
    pub spoiled: Option<i64>,
    /// The first sample after the stretch of a change, in this write.
    pub steady: Option<u64>,
}

/// Finds the first sample after a retune, and the stretch of samples that another change spoiled. It maps each sample of
/// the stream to the time of its capture: position / rate plus the least delay seen between that time and the arrival of
/// a write (the delay may rise by 1 ms per second). A write that waited on its way for a time that the stream does not
/// know ([`SettleGate::write`] with `direct` false) says nothing about the delay. The first sample of a retune that ended
/// at T is the first one captured at T + [`Settle::after`] or later, and at most [`Settle::in_flight`] samples and the
/// samples of [`Settle::after`] after the first write that follows T. Without a delay, it is that last one. A change that
/// began at B and ended at T spoils the samples captured from B to T + [`Settle::after`], with the same bounds.
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
    /// Changes that run now ([`SettleGate::begin_change`]).
    changing: u32,
    change: Option<Change>,
    /// The end of the stretch of a change, once placed. It can lie ahead of the samples written.
    steady: Option<u64>,
    /// The radio still settles ([`SettleGate::hold`]).
    held: bool,
    /// The samples since the last change, against [`HOLD_MAX_S`].
    since_change: u64,
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
            changing: 0,
            change: None,
            steady: None,
            held: false,
            since_change: 0,
        }
    }

    /// The stream started again, or its rate changed to `rate_hz`: the times of the writes before say nothing now. The
    /// stretch of a change that is not over ends at the first sample of the next write, unless a change follows.
    pub fn restart(&mut self, rate_hz: f64) {
        self.rate = rate_hz.max(1.0);
        self.pos = 0;
        self.delay = None;
        self.flush = self.marked.is_some() || self.until.is_some();
        if self.steady.is_some() {
            self.steady = Some(0);
        }
    }

    /// Retune number `seq` runs: no sample is its first one until it ends ([`SettleGate::mark`]).
    pub fn moving(&mut self, seq: u64) {
        self.marked = None;
        self.until = Some((seq, u64::MAX));
    }

    /// A retune waits for its first sample: the samples of the last write that it did not find were captured before it.
    pub fn before_move(&self) -> bool {
        self.marked.is_some() || self.until.is_some()
    }

    /// A change of the radio runs now ([`SettleGate::begin_change`]).
    pub fn changing(&self) -> bool {
        self.changing > 0
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

    /// A change of the radio other than a retune begins. No stretch ends until it ends with [`SettleGate::change`] or
    /// [`SettleGate::change_failed`].
    pub fn begin_change(&mut self) {
        self.changing += 1;
    }

    /// The change that began at `began` (a gain, a rate, another setting; the start of the stream) ended at `ended`. The
    /// next write places the stretch of the samples that it spoiled.
    pub fn change(&mut self, began: Instant, ended: Instant, settle: Settle) {
        self.step(began, ended, settle);
        self.since_change = 0;
    }

    /// As [`SettleGate::change`] for a step of a held gain that searches: the time that a hold has run on goes on.
    pub fn step(&mut self, began: Instant, ended: Instant, settle: Settle) {
        self.changing = self.changing.saturating_sub(1);
        let began = self.change.map_or(Some(began), |c| c.began.map(|b| b.min(began)));
        self.change = Some(Change {
            began,
            ended,
            settle,
        });
    }

    /// The stream started again ([`SettleGate::restart`]) and its radio was set at `ended`: the stretch of the start
    /// begins at the first sample of the next write.
    pub fn started(&mut self, ended: Instant, settle: Settle) {
        self.change(ended, ended, settle);
        if let Some(c) = &mut self.change {
            c.began = None;
        }
    }

    /// The change that began failed: the radio is as it was. Returns true if no stretch of an earlier change is open, so
    /// that the stretch of this one ends where the stream is now.
    pub fn change_failed(&mut self) -> bool {
        self.changing = self.changing.saturating_sub(1);
        self.steady.is_none() && self.change.is_none()
    }

    /// While `on`, the radio still settles (a held gain searches after a change): the stretch of a change does not end,
    /// for at most 0.5 s of samples. It changes nothing when no stretch is open.
    pub fn hold(&mut self, on: bool) {
        self.held = on;
    }

    /// `n` samples that the stream does not write pass: lost ones, or ones left out on purpose.
    pub fn pass(&mut self, n: u64) {
        self.pos += n;
        self.since_change += n;
    }

    /// The place of the first sample captured at `at` or later, from the delay estimate at time `t`.
    fn place(&self, at: Instant, t: f64) -> Option<u64> {
        let delay = self.delay.filter(|_| t - self.last <= DELAY_AGE_S)?;
        let s = at.saturating_duration_since(self.epoch).as_secs_f64();
        Some(((s - delay) * self.rate - 1e-6).ceil().max(0.0) as u64)
    }

    /// A write of `n` samples that arrived at `at` follows. `direct`: the radio sent them when it had captured the
    /// last of them, so their arrival tells the delay. Returns what lies in this write or before it.
    pub fn write(&mut self, at: Instant, n: u64, direct: bool) -> Found {
        let t = at.saturating_duration_since(self.epoch).as_secs_f64();
        let start = self.pos;
        let end = start + n;
        if direct {
            let seen = t - end as f64 / self.rate;
            let delay = match self.delay {
                None => seen,
                Some(d) => (d + DELAY_RISE * (t - self.last)).min(seen),
            };
            (self.delay, self.last) = (Some(delay), t);
        }
        let mut found = Found::default();
        let most = |s: &Settle| start + s.in_flight + (s.after.as_secs_f64() * self.rate) as u64;
        if let Some(m) = self.marked.take() {
            // The samples in flight, then the settle time: later ones were captured at the new frequency.
            let most = most(&m.settle);
            let p = self.place(m.at + m.settle.after, t).unwrap_or(most);
            self.until = Some((m.seq, p.min(most)));
        }
        if let Some(c) = self.change.take() {
            let most = most(&c.settle);
            // A place past the bound says that the estimate is off: the stretch is all that the bound allows.
            let from = c
                .began
                .and_then(|b| self.place(b, t))
                .filter(|&p| p < most)
                .unwrap_or(start);
            let to = self
                .place(c.ended + c.settle.after, t)
                .map_or(most, |p| p.min(most))
                .max(from);
            if self.steady.is_none() {
                found.spoiled = Some(from as i64 - start as i64);
            }
            self.steady = Some(self.steady.map_or(to, |s| s.max(to)));
        }
        let held = self.held && (self.since_change as f64) < HOLD_MAX_S * self.rate;
        self.pos = end;
        self.since_change += n;
        if let Some(s) = self.steady {
            if held || self.changing > 0 {
                // The stretch ends no sooner than after this write.
                self.steady = Some(s.max(end));
            } else if s <= end {
                self.steady = None;
                found.steady = Some(s.saturating_sub(start));
            }
        }
        if let Some((seq, p)) = self.until {
            if std::mem::take(&mut self.flush) {
                self.until = None;
                found.moved = Some((seq, 0));
            } else if p <= end {
                self.until = None;
                found.moved = Some((seq, p.saturating_sub(start)));
            }
        }
        found
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
            if let Some((seq, k)) = g.write(at, 10_000, true).moved {
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
        assert_eq!(g.write(Instant::now(), 10, true).moved, Some((5, 0)));
    }

    #[test]
    fn writes_that_waited_on_their_way_tell_no_delay() {
        let mut g = SettleGate::new(RATE);
        // No write came straight from the radio: the first sample is the one after all that fits in flight.
        let at = g.epoch + Duration::from_millis(500);
        assert_eq!(g.write(at, 100_000, false).moved, None);
        g.mark(1, at, settle(5, 40_000));
        assert_eq!(g.write(at, 100_000, false).moved, Some((1, 45_000)));
        // One write straight from the radio, 10 ms after its last sample; then the link falls behind by a second.
        let at = g.epoch + Duration::from_millis(310);
        assert_eq!(g.write(at, 100_000, true).moved, None);
        let late = g.epoch + Duration::from_millis(1400);
        assert_eq!(g.write(late, 100_000, false).moved, None);
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
                .moved
                .map(|(seq, k)| (seq, start + k));
        }
        assert_eq!(found, Some((2, 1_495_000)));
    }

    #[test]
    fn a_change_spoils_the_samples_captured_from_its_start_until_the_radio_settled() {
        let mut g = SettleGate::new(RATE);
        // The writes arrive 30 ms after the capture of their last sample.
        run(&mut g, 30, 1_000_000);
        // A gain change runs from 1.030 s to 1.040 s on the clock of the arrivals and settles in 5 ms: the samples
        // captured from 1.000 s to 1.015 s are spoiled, numbers 1 000 000 to 1 015 000.
        g.begin_change();
        g.change(
            g.epoch + Duration::from_millis(1030),
            g.epoch + Duration::from_millis(1040),
            settle(5, 500_000),
        );
        let at = g.epoch + Duration::from_secs_f64(1.01) + Duration::from_millis(30);
        let f = g.write(at, 10_000, true);
        assert_eq!((f.spoiled, f.steady), (Some(0), None));
        let (mut steady, mut pos) = (None, 1_010_000);
        while steady.is_none() {
            let at = g.epoch + Duration::from_secs_f64((pos + 10_000) as f64 / RATE) + Duration::from_millis(30);
            steady = g.write(at, 10_000, true).steady.map(|k| pos + k);
            pos += 10_000;
        }
        assert_eq!(steady, Some(1_015_000));
    }

    #[test]
    fn a_hold_keeps_the_stretch_of_the_start_open_until_the_search_holds() {
        let mut g = SettleGate::new(RATE);
        let t0 = g.epoch;
        g.begin_change();
        g.change(t0, t0, settle(0, 0));
        // The held gain searches during the first three writes.
        g.hold(true);
        let mut writes = Vec::new();
        for k in 1..=5u64 {
            if k == 4 {
                g.hold(false);
            }
            writes.push(g.write(t0 + Duration::from_millis(10 * k), 10_000, true));
        }
        assert_eq!(writes[0].spoiled, Some(0));
        assert!(writes[..3].iter().all(|f| f.steady.is_none()));
        // The first sample of the fourth write is the first one after the search.
        assert_eq!(writes[3].steady, Some(0));
        assert_eq!(writes[4], Found::default());
    }

    #[test]
    fn a_hold_ends_after_half_a_second_of_samples() {
        let mut g = SettleGate::new(RATE);
        let t0 = g.epoch;
        g.begin_change();
        g.change(t0, t0, settle(0, 0));
        g.hold(true);
        let mut end = None;
        for k in 1..=300u64 {
            let f = g.write(t0 + Duration::from_millis(10 * k), 10_000, true);
            if let Some(s) = f.steady {
                end = Some((k - 1) * 10_000 + s);
                break;
            }
        }
        assert_eq!(end, Some(500_000));
    }

    #[test]
    fn a_search_that_holds_with_its_step_ends_the_stretch_where_the_step_lands() {
        let mut g = SettleGate::new(RATE);
        let t0 = g.epoch;
        g.begin_change();
        g.change(t0, t0, settle(0, 0));
        g.hold(true);
        // Writes of 10 000 samples that arrive 30 ms after the capture of their last sample.
        let write = |g: &mut SettleGate| {
            let start = g.pos;
            let at = t0 + Duration::from_secs_f64((start + 10_000) as f64 / RATE) + Duration::from_millis(30);
            g.write(at, 10_000, true).steady.map(|k| start + k)
        };
        while g.pos < 200_000 {
            assert_eq!(write(&mut g), None);
        }
        // The search steps at 0.23 s on the clock of the arrivals and holds at once: the sample captured 5 ms after the step
        // is number 205 000.
        let at = t0 + Duration::from_millis(230);
        g.begin_change();
        g.step(at, at, settle(5, 500_000));
        g.hold(false);
        assert_eq!(write(&mut g), Some(205_000));
    }

    #[test]
    fn no_sample_is_the_first_one_of_a_retune_while_it_runs() {
        let mut g = SettleGate::new(RATE);
        run(&mut g, 0, 100_000);
        g.moving(1);
        assert!(g.before_move());
        assert_eq!(run(&mut g, 0, 300_000), None);
        g.mark(1, g.epoch + Duration::from_millis(305), settle(5, 500_000));
        assert_eq!(run(&mut g, 0, 400_000), Some((1, 310_000)));
        assert!(!g.before_move());
    }

    #[test]
    fn a_failed_change_ends_its_stretch_where_the_stream_is() {
        let mut g = SettleGate::new(RATE);
        g.begin_change();
        assert!(g.change_failed());
        assert_eq!(g.write(g.epoch, 10, true), Found::default());
    }

    #[test]
    fn the_steps_of_a_search_do_not_extend_a_hold() {
        let mut g = SettleGate::new(RATE);
        let t0 = g.epoch;
        g.begin_change();
        g.change(t0, t0, settle(0, 0));
        g.hold(true);
        let mut end = None;
        for k in 1..=300u64 {
            let at = t0 + Duration::from_millis(10 * k);
            if k % 30 == 0 {
                g.begin_change();
                g.step(at, at, settle(5, 0));
            }
            if let Some(s) = g.write(at, 10_000, true).steady {
                end = Some((k - 1) * 10_000 + s);
                break;
            }
        }
        assert_eq!(end, Some(500_000));
    }
}
