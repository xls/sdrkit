//! The held automatic gain of a driver. The driver keeps the radio in manual gain and runs this control on the level of the
//! samples it reads: it chooses the gain that puts the peak [`RULES`] `target_peak_dbfs` below full scale, steps once, waits
//! until the step shows in the samples, and then holds. It moves again only when the level stays outside a wide window for a
//! set time, and never sooner than a set time after its last step. The clock is the count of samples, so a backlog or a fast
//! test source keeps the same timing.

use sdrkit_core::publish::field;
use sdrkit_core::Field;

/// The rules of the control.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoldRules {
    /// The peak of the I and Q components that the control aims at, in dBFS.
    pub target_peak_dbfs: f64,
    /// A search ends when the peak is this close to the target, in dB.
    pub search_tolerance_db: f64,
    /// A held gain moves only when the peak is more than this off the target, or the samples clip, in dB.
    pub hold_window_db: f64,
    /// ... for longer than this, in s.
    pub out_for_s: f64,
    /// ... and no sooner than this after the last step, in s.
    pub min_interval_s: f64,
    /// The length of one measurement window, in s.
    pub window_s: f64,
    /// The peak-to-mean ratio taken for a clipped window before the control has measured one, in dB.
    pub peak_to_mean_db: f64,
}

/// The rules of every driver: peak at -10 dBFS (within 3 dB after a search), hold while the peak stays within 6 dB of it,
/// move after 0.5 s outside, at most once in 2 s, windows of 100 ms.
pub const RULES: HoldRules = HoldRules {
    target_peak_dbfs: -10.0,
    search_tolerance_db: 3.0,
    hold_window_db: 6.0,
    out_for_s: 0.5,
    min_interval_s: 2.0,
    window_s: 0.1,
    peak_to_mean_db: 13.0,
};

sdrkit_core::choice_enum! {
    /// What the held gain does now.
    pub enum GainControl {
        /// The control looks for the gain: it steps and waits for each step to show.
        Searching = ("searching", "Searching"),
        /// The gain is held.
        Holding = ("holding", "Holding"),
    }
}

/// What the held gain of one receiver publishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HoldStatus {
    /// The gain now, in dB.
    pub gain_db: f64,
    /// Searching or holding.
    pub state: GainControl,
    /// The gain steps since the last start (a retune).
    pub steps: u64,
}

/// The fields of the held gain of one receiver.
#[derive(Clone, Copy, Debug)]
pub struct HoldFields {
    /// The gain now.
    pub gain: Field<field::F64>,
    /// Searching or holding.
    pub state: Field<field::Choice<GainControl>>,
    /// The steps since the last retune.
    pub steps: Field<field::I64>,
}

impl HoldFields {
    /// Writes `status` into `f`; every field is absent when the gain is not held.
    pub fn write(&self, f: &mut sdrkit_core::RowMut<'_>, status: Option<HoldStatus>) {
        f.opt(&self.gain, status.map(|s| s.gain_db));
        f.opt(&self.state, status.map(|s| s.state));
        f.opt(
            &self.steps,
            status.map(|s| i64::try_from(s.steps).unwrap_or(i64::MAX)),
        );
    }
}

/// The level of one window of samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    /// The largest I or Q, in dBFS.
    pub peak_dbfs: f64,
    /// The mean power of one component, in dBFS.
    pub mean_dbfs: f64,
    /// More than one in 100 000 values sat at full scale.
    pub clipped: bool,
    /// The sample instants in the window.
    pub samples: u64,
}

/// The lowest level that a window reports, in dBFS (a window of zeros).
const FLOOR_DBFS: f64 = -150.0;

/// Peak, mean and clipping of the I and Q components over windows of a set number of samples.
#[derive(Clone, Debug)]
pub struct Meter {
    window: u64,
    full: f64,
    clip_at: i32,
    n: u64,
    peak: i32,
    sum_sq: f64,
    clipped: u64,
}

impl Meter {
    /// A meter of windows of `window` samples, for integer components of full scale `full` (2048 for 12 bits).
    pub fn new(window: u64, full: i32) -> Meter {
        Meter {
            window: window.max(1),
            full: f64::from(full.max(1)),
            clip_at: (full - 1).max(1),
            n: 0,
            peak: 0,
            sum_sq: 0.0,
            clipped: 0,
        }
    }

    /// Adds one sample. It returns the level when the sample completes a window.
    #[inline]
    pub fn add(&mut self, i: i32, q: i32) -> Option<Level> {
        let (a, b) = (i.saturating_abs(), q.saturating_abs());
        let m = a.max(b);
        self.peak = self.peak.max(m);
        self.sum_sq += f64::from(i) * f64::from(i) + f64::from(q) * f64::from(q);
        if m >= self.clip_at {
            self.clipped += u64::from(a >= self.clip_at) + u64::from(b >= self.clip_at);
        }
        self.n += 1;
        (self.n >= self.window).then(|| self.take())
    }

    fn take(&mut self) -> Level {
        let n = self.n as f64;
        let db = |x: f64| {
            if x > 0.0 {
                (10.0 * x.log10()).max(FLOOR_DBFS)
            } else {
                FLOOR_DBFS
            }
        };
        let level = Level {
            peak_dbfs: db((f64::from(self.peak) / self.full).powi(2)),
            mean_dbfs: db(self.sum_sq / (2.0 * n) / (self.full * self.full)),
            clipped: self.clipped * 100_000 > 2 * self.n,
            samples: self.n,
        };
        (self.n, self.peak, self.sum_sq, self.clipped) = (0, 0, 0.0, 0);
        level
    }
}

/// What the control waits for before it looks at the level again.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Wait {
    None,
    /// A number of samples (the samples in flight after a start).
    For(u64),
    /// A step of `expect_db` to show in the mean against `before_dbfs`, or `left` samples.
    Step {
        expect_db: f64,
        before_dbfs: f64,
        left: u64,
    },
}

/// The held gain of one receiver: a state machine fed with the level of each window. It proposes a gain; the driver writes
/// it and reports the write with [`HeldGain::stepped`].
#[derive(Clone, Debug)]
pub struct HeldGain {
    rules: HoldRules,
    active: bool,
    gain_db: f64,
    range: (f64, f64),
    state: GainControl,
    steps: u64,
    window: u64,
    latency: u64,
    since_step: u64,
    out: u64,
    wait: Wait,
    peak_to_mean_db: f64,
    /// The mean of the last window, against which a step is seen.
    last_mean_dbfs: f64,
    /// The first search of the stream has not held yet ([`HeldGain::settling`]).
    first: bool,
}

impl Default for HeldGain {
    fn default() -> Self {
        HeldGain::new(RULES)
    }
}

impl HeldGain {
    /// A control with `rules`, off until [`HeldGain::start`].
    pub fn new(rules: HoldRules) -> HeldGain {
        HeldGain {
            rules,
            active: false,
            gain_db: 0.0,
            range: (0.0, 0.0),
            state: GainControl::Searching,
            steps: 0,
            window: 1,
            latency: 0,
            since_step: 0,
            out: 0,
            wait: Wait::None,
            peak_to_mean_db: rules.peak_to_mean_db,
            last_mean_dbfs: FLOOR_DBFS,
            first: false,
        }
    }

    /// The samples of one window at `rate_hz`.
    pub fn window_samples(&self, rate_hz: f64) -> u64 {
        samples(rate_hz, self.rules.window_s).max(1)
    }

    /// Starts the control (at open and after a retune): the radio was set to `gain_db`, of `range` (lowest, highest) in dB, at
    /// `rate_hz`. `latency` is the samples that can be in flight between the radio and the meter; the control waits for them
    /// before it measures, and after each step.
    pub fn start(&mut self, gain_db: f64, range: (f64, f64), rate_hz: f64, latency: u64) {
        self.active = true;
        self.gain_db = gain_db;
        self.range = range;
        self.state = GainControl::Searching;
        self.steps = 0;
        self.window = self.window_samples(rate_hz);
        self.latency = latency;
        self.since_step = 0;
        self.out = 0;
        self.wait = Wait::For(latency + self.window);
        self.peak_to_mean_db = self.rules.peak_to_mean_db;
    }

    /// A stream starts: its first search, also one that a retune starts again before it holds, is
    /// [`HeldGain::settling`].
    pub fn stream_started(&mut self) {
        self.first = self.active && self.state == GainControl::Searching;
    }

    /// Every sample that the meter takes from now on was captured after the gain was written (a new stream, or a driver
    /// that leaves out the samples of before a retune): a search that has not stepped measures the next window.
    pub fn fresh(&mut self) {
        if self.active && self.steps == 0 && matches!(self.wait, Wait::For(_)) {
            self.wait = Wait::None;
        }
    }

    /// The first search of the stream runs: the level of the samples is not yet the one the control holds. Later searches
    /// spoil only the samples around each step.
    pub fn settling(&self) -> bool {
        self.active && self.first
    }

    /// Turns the control off (manual gain, or the AGC of the radio).
    pub fn stop(&mut self) {
        self.active = false;
        self.first = false;
    }

    /// The control runs.
    pub fn active(&self) -> bool {
        self.active
    }

    /// What it publishes; `None` when it is off.
    pub fn status(&self) -> Option<HoldStatus> {
        self.active.then_some(HoldStatus {
            gain_db: self.gain_db,
            state: self.state,
            steps: self.steps,
        })
    }

    /// Takes the level of one window. It returns the gain to write, if the control steps now.
    pub fn feed(&mut self, l: &Level) -> Option<f64> {
        if !self.active {
            return None;
        }
        self.since_step = self.since_step.saturating_add(l.samples);
        self.last_mean_dbfs = l.mean_dbfs;
        match &mut self.wait {
            Wait::None => {}
            // The window that ends a wait can still hold samples of before: it is not measured either.
            Wait::For(left) => {
                if *left > l.samples {
                    *left -= l.samples;
                } else {
                    self.wait = Wait::None;
                }
                return None;
            }
            Wait::Step {
                expect_db,
                before_dbfs,
                left,
            } => {
                let moved = l.mean_dbfs - *before_dbfs;
                let need = expect_db.abs().min(6.0) / 2.0;
                let seen = if *expect_db < 0.0 {
                    moved <= -need
                } else {
                    moved >= need
                };
                if seen || *left <= l.samples {
                    // A step that does not show leaves the level as it was: the search no longer spoils it.
                    self.first &= seen;
                    self.wait = Wait::None;
                } else {
                    *left -= l.samples;
                }
                return None;
            }
        }
        match self.state {
            GainControl::Searching => self.search(l),
            GainControl::Holding => {
                let r = &self.rules;
                let off = l.clipped || (l.peak_dbfs - r.target_peak_dbfs).abs() > r.hold_window_db;
                self.out = if off { self.out + l.samples } else { 0 };
                let rate = self.window as f64 / r.window_s;
                if self.out >= samples(rate, r.out_for_s)
                    && self.since_step >= samples(rate, r.min_interval_s)
                {
                    self.state = GainControl::Searching;
                    self.search(l)
                } else {
                    None
                }
            }
        }
    }

    /// The gain that puts the peak of `l` at the target, or a hold if the peak is close enough or the gain is at its limit.
    fn search(&mut self, l: &Level) -> Option<f64> {
        if !l.clipped {
            self.peak_to_mean_db = (l.peak_dbfs - l.mean_dbfs).clamp(3.0, 20.0);
        }
        // A clipped window has no peak: the mean and the last peak-to-mean ratio give a peak that is too low, so the next step
        // corrects the rest.
        let peak = if l.clipped {
            l.mean_dbfs + self.peak_to_mean_db
        } else {
            l.peak_dbfs
        };
        let err = self.rules.target_peak_dbfs - peak;
        if !l.clipped && err.abs() <= self.rules.search_tolerance_db {
            self.hold();
            return None;
        }
        let mut want = (self.gain_db + err).round();
        if l.clipped {
            want = want.min(self.gain_db - 3.0);
        }
        let want = want.clamp(self.range.0, self.range.1);
        if (want - self.gain_db).abs() < 0.5 {
            self.hold();
            return None;
        }
        Some(want)
    }

    fn hold(&mut self) {
        self.first = false;
        self.state = GainControl::Holding;
        self.out = 0;
        self.wait = Wait::None;
    }

    /// The driver wrote `gain_db`, the gain that [`HeldGain::feed`] proposed. The control waits until the step shows in the mean,
    /// or for twice the samples in flight and five windows.
    pub fn stepped(&mut self, gain_db: f64) {
        self.wait = Wait::Step {
            expect_db: gain_db - self.gain_db,
            before_dbfs: self.last_mean_dbfs,
            left: 2 * self.latency + 5 * self.window,
        };
        self.gain_db = gain_db;
        self.steps += 1;
        self.since_step = 0;
        // A step to a limit of the gain is the last one the search can make: the level after it is the one it holds.
        if gain_db <= self.range.0 || gain_db >= self.range.1 {
            self.first = false;
        }
    }

    /// The driver could not write the proposed gain: the control waits ten windows and looks again.
    pub fn not_stepped(&mut self) {
        self.wait = Wait::For(10 * self.window);
    }

}

fn samples(rate_hz: f64, s: f64) -> u64 {
    (rate_hz * s).round().max(0.0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 1e6;

    /// A window at `peak` dBFS with a peak-to-mean ratio of 13 dB, clipped above full scale.
    fn at(peak: f64) -> Level {
        Level {
            peak_dbfs: peak.min(0.0),
            mean_dbfs: (peak - 13.0).min(-1.0),
            clipped: peak > 0.0,
            samples: 100_000,
        }
    }

    /// Feeds windows of a signal whose peak at gain `g` dB is `input(g)` dBFS, writes each proposal at once, and returns the
    /// windows it took to hold with more than `after` steps.
    fn run(g: &mut HeldGain, input: impl Fn(f64) -> f64, after: u64, max: usize) -> Option<usize> {
        for k in 0..max {
            if let Some(w) = g.feed(&at(input(g.gain_db))) {
                g.stepped(w);
            }
            if g
                .status()
                .is_some_and(|s| s.state == GainControl::Holding && s.steps > after)
            {
                return Some(k);
            }
        }
        None
    }

    #[test]
    fn a_search_reaches_the_target_from_far_above_and_far_below() {
        for start_err in [30.0, -30.0, 20.0] {
            let mut g = HeldGain::default();
            g.start(40.0, (0.0, 73.0), RATE, 0);
            let input = move |gain: f64| -10.0 + start_err + (gain - 40.0);
            let k = run(&mut g, input, 0, 40).expect("holds");
            let s = g.status().unwrap();
            assert!((input(s.gain_db) + 10.0).abs() <= 3.0, "{start_err}: {s:?}");
            assert!(s.steps <= 4 && k <= 20, "{start_err}: {k} windows, {s:?}");
        }
    }

    #[test]
    fn a_held_gain_ignores_short_excursions_and_follows_a_lasting_change() {
        let mut g = HeldGain::default();
        g.start(40.0, (0.0, 73.0), RATE, 0);
        run(&mut g, |gain| -4.0 + gain - 40.0, 0, 40).unwrap();
        let held = g.status().unwrap();
        // 0.4 s clipped, then back: no step.
        for k in 0..100 {
            let peak = if (20..24).contains(&k) { 10.0 } else { -10.0 };
            assert_eq!(g.feed(&at(peak)), None);
        }
        assert_eq!(g.status(), Some(held));
        // 20 dB stronger for good: a step after 0.5 s, then a hold near the target.
        let input = move |gain: f64| 10.0 + gain - held.gain_db;
        let k = run(&mut g, input, held.steps, 60).unwrap();
        let s = g.status().unwrap();
        assert!((input(s.gain_db) + 10.0).abs() <= 3.0, "{s:?}");
        assert!(k <= 30, "{k} windows");
    }

    #[test]
    fn a_gain_at_its_limit_holds_without_steps() {
        let mut g = HeldGain::default();
        g.start(73.0, (0.0, 73.0), RATE, 0);
        for _ in 0..100 {
            assert_eq!(g.feed(&at(-60.0)), None);
        }
        assert_eq!(g.status().unwrap().steps, 0);
        assert_eq!(g.status().unwrap().state, GainControl::Holding);
    }

    #[test]
    fn a_meter_measures_peak_mean_and_clipping() {
        let mut m = Meter::new(4, 2048);
        assert_eq!(m.add(1024, 0), None);
        assert_eq!(m.add(-1024, 0), None);
        assert_eq!(m.add(0, 1024), None);
        let l = m.add(0, -1024).unwrap();
        assert!((l.peak_dbfs + 6.02).abs() < 0.01, "{l:?}");
        assert!((l.mean_dbfs + 9.03).abs() < 0.01, "{l:?}");
        assert!(!l.clipped);
        for _ in 0..3 {
            m.add(2047, -2048);
        }
        assert!(m.add(0, 0).unwrap().clipped);
    }
}
