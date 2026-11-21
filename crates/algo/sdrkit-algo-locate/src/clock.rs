//! Timing sources and the disciplined sample clock.
//!
//! A receiver counts samples; a timing source says now and then which UTC instant belongs to a sample time (a mark). The sample
//! clock fits UTC as a straight line of the sample time over the recent marks, so any sample can be stamped with UTC and a 1-sigma
//! error. The sample time is the sample count divided by the nominal rate, in seconds.
//!
//! The fit is a weighted least-squares line with a prior on its rate of 1 +/- 50 ppm, so one mark already stamps (with the rate
//! error in its sigma). Marks that disagree with the line are refused; three refused in a row restart the clock from the newest.
//! [`ClockSet`] holds one clock per source and stamps with the best usable one.

use std::collections::VecDeque;

use crate::LocateError;

/// A timing source, best first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TimeSource {
    /// The radio's own GPS time: a hardware timestamp gives UTC at a sample index.
    Gps,
    /// A GNSS receiver's fix moved onto this radio's sample time.
    Gnss,
    /// The symbol clock of a timing transmitter that all receivers hear (see [`crate::commonview`]).
    CommonView,
    /// The computer clock, disciplined by PTP, read when a chunk of samples arrives.
    Ptp,
    /// The computer clock, disciplined by NTP, read when a chunk of samples arrives.
    Ntp,
    /// The computer clock without discipline.
    System,
}

/// A detection is fine when its source is fine-capable and its 1-sigma is at most this, s.
pub const FINE_SIGMA_S: f64 = 20e-6;

/// The most marks a clock holds.
pub const MAX_MARKS: usize = 256;

/// The 1-sigma of the rate prior (the rate is 1 within this).
const RATE_PRIOR_SIGMA: f64 = 50e-6;

/// The least mark sigma in the weights, s.
const MIN_MARK_SIGMA_S: f64 = 1e-9;

/// A mark whose residual exceeds `REFUSE_SIGMAS` sigmas + `REFUSE_RMS` rms + `REFUSE_MARGIN_S` is refused.
const REFUSE_SIGMAS: f64 = 6.0;
const REFUSE_RMS: f64 = 3.0;
const REFUSE_MARGIN_S: f64 = 50e-9;

/// Refused marks in a row that restart the clock.
const REFUSALS_TO_RESTART: u32 = 3;

/// A mark earlier than the last one by more than this (sample time) restarts the clock, s.
const BACKWARDS_S: f64 = 1e-6;

impl TimeSource {
    /// Every source, best first.
    pub const ALL: [TimeSource; 6] = [
        TimeSource::Gps,
        TimeSource::Gnss,
        TimeSource::CommonView,
        TimeSource::Ptp,
        TimeSource::Ntp,
        TimeSource::System,
    ];

    /// The rank, 1 (best) to 6.
    pub fn rank(self) -> u8 {
        match self {
            TimeSource::Gps => 1,
            TimeSource::Gnss => 2,
            TimeSource::CommonView => 3,
            TimeSource::Ptp => 4,
            TimeSource::Ntp => 5,
            TimeSource::System => 6,
        }
    }

    /// The source of a rank (1 to 6).
    pub fn from_rank(rank: u8) -> Option<TimeSource> {
        TimeSource::ALL
            .get(usize::from(rank).wrapping_sub(1))
            .copied()
    }

    /// The name in plain words.
    pub fn name(self) -> &'static str {
        match self {
            TimeSource::Gps => "GPS",
            TimeSource::Gnss => "GNSS",
            TimeSource::CommonView => "common view",
            TimeSource::Ptp => "PTP",
            TimeSource::Ntp => "NTP",
            TimeSource::System => "system clock",
        }
    }

    /// The typical 1-sigma of the source before any measurement, s: 100 ns, 1 us, 3 us, 100 us, 5 ms, 200 ms.
    pub fn typical_sigma_s(self) -> f64 {
        match self {
            TimeSource::Gps => 100e-9,
            TimeSource::Gnss => 1e-6,
            TimeSource::CommonView => 3e-6,
            TimeSource::Ptp => 100e-6,
            TimeSource::Ntp => 5e-3,
            TimeSource::System => 200e-3,
        }
    }

    /// Whether detections stamped by this source can be fine (GPS, GNSS, common view).
    pub fn fine_capable(self) -> bool {
        self.rank() <= 3
    }

    /// The span of marks that the clock of this source fits, s: 60 for GPS and GNSS, 300 for common view, 120 for the computer
    /// clock (PTP, NTP, system).
    pub fn window_s(self) -> f64 {
        match self {
            TimeSource::Gps | TimeSource::Gnss => 60.0,
            TimeSource::CommonView => 300.0,
            TimeSource::Ptp | TimeSource::Ntp | TimeSource::System => 120.0,
        }
    }

    /// The time without a mark after which the clock is no longer used, s: 5 for GPS and GNSS, none for the others.
    pub fn stale_after_s(self) -> Option<f64> {
        match self {
            TimeSource::Gps | TimeSource::Gnss => Some(5.0),
            _ => None,
        }
    }
}

/// Whether a detection stamped by `source` with the 1-sigma `sigma_s` is fine: a fine-capable source and at most
/// [`FINE_SIGMA_S`].
pub fn is_fine(source: TimeSource, sigma_s: f64) -> bool {
    source.fine_capable() && sigma_s <= FINE_SIGMA_S
}

/// One time mark: the UTC instant of a sample time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    /// The source that made the mark.
    pub source: TimeSource,
    /// The sample time (sample count / nominal rate), s.
    pub sample_s: f64,
    /// UTC, ns since 1970-01-01.
    pub utc_ns: i64,
    /// The random 1-sigma of the mark, s (above 0).
    pub sigma_s: f64,
    /// The fixed bias that the mark may carry, s (at least 0). It does not average out over marks.
    pub bias_s: f64,
}

/// What [`SampleClock::add`] did with a mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkOutcome {
    /// The mark joined the fit.
    Accepted,
    /// The mark disagreed with the fit and was left out.
    Refused,
    /// The clock forgot its marks and started again from this one.
    Restarted,
}

/// A UTC stamp with its 1-sigma.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stamp {
    /// UTC, ns since 1970-01-01.
    pub utc_ns: i64,
    /// The 1-sigma, s, bias included.
    pub sigma_s: f64,
}

/// The fitted line. Times are held relative to the newest mark, so `f64` keeps nanoseconds.
#[derive(Clone, Copy, Debug)]
struct Line {
    /// Sample time of the newest mark, s.
    s0: f64,
    /// UTC of the newest mark, ns.
    u0: i64,
    /// The weighted mean sample time, relative to `s0`, s.
    xm: f64,
    /// UTC at `xm`, relative to `u0`, s.
    a: f64,
    /// UTC seconds per sample second.
    b: f64,
    var_a: f64,
    var_b: f64,
    bias: f64,
    rms: f64,
}

/// The disciplined sample clock of one source.
#[derive(Clone, Debug, Default)]
pub struct SampleClock {
    source: Option<TimeSource>,
    marks: VecDeque<Mark>,
    line: Option<Line>,
    refused_in_row: u32,
    refused: u64,
    restarts: u64,
}

impl SampleClock {
    /// An empty clock; the first mark sets its source.
    pub fn new() -> Self {
        Self::default()
    }

    /// The source of the marks held, if any.
    pub fn source(&self) -> Option<TimeSource> {
        self.source
    }

    /// Marks held.
    pub fn marks(&self) -> usize {
        self.marks.len()
    }

    /// Marks refused since the clock was made or reset.
    pub fn refused(&self) -> u64 {
        self.refused
    }

    /// Restarts since the clock was made or reset.
    pub fn restarts(&self) -> u64 {
        self.restarts
    }

    /// The sample time of the newest mark, s.
    pub fn last_sample_s(&self) -> Option<f64> {
        self.marks.back().map(|m| m.sample_s)
    }

    /// Forgets the marks and the counters.
    pub fn reset(&mut self) {
        *self = SampleClock::default();
    }

    /// Adds a mark. A mark from another source than the marks held, or one whose sample time is more than 1 us before the newest
    /// mark's, restarts the clock. With 3 or more marks held, a mark whose residual from the line exceeds `6 sqrt(stamp sigma^2 -
    /// bias^2 + mark sigma^2) + 3 rms + 50 ns` is refused; the third refusal in a row restarts the clock from that mark. Old marks
    /// leave the fit after the source's window ([`TimeSource::window_s`]) and beyond [`MAX_MARKS`]. Errors for a mark with a
    /// non-finite sample time, a sigma that is not above 0 or a negative bias.
    pub fn add(&mut self, mark: Mark) -> Result<MarkOutcome, LocateError> {
        if !mark.sample_s.is_finite() {
            return LocateError::range("mark sample time", mark.sample_s);
        }
        if !(mark.sigma_s > 0.0 && mark.sigma_s.is_finite()) {
            return LocateError::range("mark sigma", mark.sigma_s);
        }
        if !(mark.bias_s >= 0.0 && mark.bias_s.is_finite()) {
            return LocateError::range("mark bias", mark.bias_s);
        }
        let jumped = match (self.source, self.marks.back()) {
            (Some(src), Some(last)) => {
                src != mark.source || mark.sample_s < last.sample_s - BACKWARDS_S
            }
            _ => false,
        };
        if jumped {
            self.restart(mark);
            return Ok(MarkOutcome::Restarted);
        }
        if self.marks.len() >= 3 {
            if let Some(line) = self.line {
                let (rel, sigma) = line.at(mark.sample_s);
                let residual = mark.utc_ns.saturating_sub(line.u0) as f64 * 1e-9 - rel;
                let spread =
                    (sigma * sigma - line.bias * line.bias).max(0.0) + mark.sigma_s * mark.sigma_s;
                let limit = REFUSE_SIGMAS * spread.sqrt() + REFUSE_RMS * line.rms + REFUSE_MARGIN_S;
                if residual.abs() > limit {
                    self.refused += 1;
                    self.refused_in_row += 1;
                    if self.refused_in_row >= REFUSALS_TO_RESTART {
                        self.restart(mark);
                        return Ok(MarkOutcome::Restarted);
                    }
                    return Ok(MarkOutcome::Refused);
                }
            }
        }
        self.refused_in_row = 0;
        self.source = Some(mark.source);
        self.marks.push_back(mark);
        let oldest = mark.sample_s - mark.source.window_s();
        while self.marks.len() > MAX_MARKS
            || self.marks.front().is_some_and(|m| m.sample_s < oldest)
        {
            self.marks.pop_front();
        }
        self.fit();
        Ok(MarkOutcome::Accepted)
    }

    /// UTC and its 1-sigma at the sample time `sample_s`: `UTC = a + b (s - s_mean)`, sigma `sqrt(var(a) + (s - s_mean)^2 var(b) +
    /// bias^2)`. None without marks.
    pub fn stamp(&self, sample_s: f64) -> Option<Stamp> {
        let line = self.line?;
        let (rel, sigma) = line.at(sample_s);
        Some(Stamp {
            utc_ns: line.u0.saturating_add((rel * 1e9).round() as i64),
            sigma_s: sigma,
        })
    }

    /// The rate error of the sample clock, ppm: `(1 / b - 1) 1e6`, positive when the radio runs fast. None without marks.
    pub fn ppm(&self) -> Option<f64> {
        self.line.map(|l| (1.0 / l.b - 1.0) * 1e6)
    }

    /// The 1-sigma of [`SampleClock::ppm`], ppm.
    pub fn ppm_sigma(&self) -> Option<f64> {
        self.line.map(|l| l.var_b.sqrt() / (l.b * l.b) * 1e6)
    }

    /// Whether the clock can stamp at the sample time `now_s`: it holds marks and, for GPS and GNSS, the newest is at most 5 s old.
    pub fn usable(&self, now_s: f64) -> bool {
        match (self.source, self.marks.back()) {
            (Some(src), Some(last)) => src
                .stale_after_s()
                .is_none_or(|t| now_s - last.sample_s <= t),
            _ => false,
        }
    }

    fn restart(&mut self, mark: Mark) {
        if !self.marks.is_empty() {
            self.restarts += 1;
        }
        self.marks.clear();
        self.refused_in_row = 0;
        self.source = Some(mark.source);
        self.marks.push_back(mark);
        self.fit();
    }

    /// The weighted least-squares line through the marks held, with the rate prior; variances scaled by chi^2 / (n - 2) when
    /// that exceeds 1 with 3 or more marks.
    fn fit(&mut self) {
        let Some(newest) = self.marks.back().copied() else {
            self.line = None;
            return;
        };
        let (s0, u0) = (newest.sample_s, newest.utc_ns);
        let point = |m: &Mark| {
            let w = 1.0 / m.sigma_s.max(MIN_MARK_SIGMA_S).powi(2);
            (
                m.sample_s - s0,
                m.utc_ns.saturating_sub(u0) as f64 * 1e-9,
                w,
            )
        };
        let (mut sw, mut swx, mut swy) = (0.0, 0.0, 0.0);
        for (x, y, w) in self.marks.iter().map(point) {
            sw += w;
            swx += w * x;
            swy += w * y;
        }
        let xm = swx / sw;
        let a = swy / sw;
        let (mut sxx, mut sxy) = (0.0, 0.0);
        for (x, y, w) in self.marks.iter().map(point) {
            sxx += w * (x - xm) * (x - xm);
            sxy += w * (x - xm) * y;
        }
        let prior = 1.0 / (RATE_PRIOR_SIGMA * RATE_PRIOR_SIGMA);
        let b = (sxy + prior) / (sxx + prior);
        let mut var_a = 1.0 / sw;
        let mut var_b = 1.0 / (sxx + prior);
        let (mut chi2, mut sq) = (0.0, 0.0);
        for (x, y, w) in self.marks.iter().map(point) {
            let r = y - a - b * (x - xm);
            chi2 += w * r * r;
            sq += r * r;
        }
        let n = self.marks.len();
        if n >= 3 {
            let scale = chi2 / (n - 2) as f64;
            if scale > 1.0 {
                var_a *= scale;
                var_b *= scale;
            }
        }
        self.line = Some(Line {
            s0,
            u0,
            xm,
            a,
            b,
            var_a,
            var_b,
            bias: self.marks.iter().map(|m| m.bias_s).fold(0.0, f64::max),
            rms: (sq / n as f64).sqrt(),
        });
    }
}

impl Line {
    /// UTC relative to `u0` (s) and its 1-sigma at a sample time.
    fn at(&self, sample_s: f64) -> (f64, f64) {
        let dx = sample_s - self.s0 - self.xm;
        let rel = self.a + self.b * dx;
        let sigma = (self.var_a + dx * dx * self.var_b + self.bias * self.bias).sqrt();
        (rel, sigma)
    }
}

/// One [`SampleClock`] per timing source.
#[derive(Clone, Debug, Default)]
pub struct ClockSet {
    clocks: [SampleClock; 6],
}

impl ClockSet {
    /// Six empty clocks.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a mark to the clock of its source (see [`SampleClock::add`]).
    pub fn add(&mut self, mark: Mark) -> Result<MarkOutcome, LocateError> {
        self.clocks[usize::from(mark.source.rank() - 1)].add(mark)
    }

    /// The clock of one source.
    pub fn clock(&self, source: TimeSource) -> &SampleClock {
        &self.clocks[usize::from(source.rank() - 1)]
    }

    /// Forgets the marks of one source.
    pub fn reset_source(&mut self, source: TimeSource) {
        self.clocks[usize::from(source.rank() - 1)].reset();
    }

    /// Forgets every mark.
    pub fn reset(&mut self) {
        for c in &mut self.clocks {
            c.reset();
        }
    }

    /// The best source whose clock is usable at the sample time `now_s`.
    pub fn best(&self, now_s: f64) -> Option<TimeSource> {
        TimeSource::ALL
            .into_iter()
            .find(|&s| self.clock(s).usable(now_s))
    }

    /// The stamp of `sample_s` by the best source usable at `now_s`, with that source.
    pub fn stamp(&self, now_s: f64, sample_s: f64) -> Option<(TimeSource, Stamp)> {
        let source = self.best(now_s)?;
        self.clock(source).stamp(sample_s).map(|s| (source, s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_sim::rng::Rng;

    const T0_NS: i64 = 1_700_000_000_000_000_000;

    fn gps(sample_s: f64, utc_ns: i64) -> Mark {
        Mark {
            source: TimeSource::Gps,
            sample_s,
            utc_ns,
            sigma_s: 30e-9,
            bias_s: 0.0,
        }
    }

    #[test]
    fn gps_marks_give_rate_and_stamps_then_a_jump_restarts() {
        // 60 marks one UTC second apart; the radio runs 23.7 ppm fast; 30 ns of jitter on each mark.
        let rate = 1.0 + 23.7e-6;
        let s_start = 1234.5;
        let mut rng = Rng::new(42);
        let mut clock = SampleClock::new();
        for k in 0..60 {
            let utc = T0_NS + k * 1_000_000_000 + (30.0 * rng.normal()).round() as i64;
            assert_eq!(
                clock.add(gps(s_start + k as f64 * rate, utc)).unwrap(),
                MarkOutcome::Accepted
            );
        }
        assert!(
            (clock.ppm().unwrap() - 23.7).abs() < 0.01,
            "{:?}",
            clock.ppm()
        );
        assert!(clock.ppm_sigma().unwrap() < 0.01);
        let st = clock.stamp(s_start + 30.5 * rate).unwrap();
        let truth = T0_NS + 30_500_000_000;
        assert!((st.utc_ns - truth).abs() < 40, "{}", st.utc_ns - truth);
        assert!(st.sigma_s < 40e-9, "{}", st.sigma_s);
        assert!(clock.usable(s_start + 62.0) && !clock.usable(s_start + 66.0));
        // A mark 5 ms off is refused; the third in a row restarts the clock from it.
        let off = |k: i64| {
            gps(
                s_start + k as f64 * rate,
                T0_NS + k * 1_000_000_000 + 5_000_000,
            )
        };
        assert_eq!(clock.add(off(60)).unwrap(), MarkOutcome::Refused);
        assert_eq!(clock.add(off(61)).unwrap(), MarkOutcome::Refused);
        assert_eq!(clock.add(off(62)).unwrap(), MarkOutcome::Restarted);
        assert_eq!(
            (clock.restarts(), clock.marks(), clock.refused()),
            (1, 1, 3)
        );
        // A mark that goes back in sample time, or one from another source, restarts too.
        assert_eq!(clock.add(off(10)).unwrap(), MarkOutcome::Restarted);
        let ntp = Mark {
            source: TimeSource::Ntp,
            ..off(11)
        };
        assert_eq!(clock.add(ntp).unwrap(), MarkOutcome::Restarted);
        assert_eq!(clock.source(), Some(TimeSource::Ntp));
        assert!(clock
            .add(Mark {
                sigma_s: 0.0,
                ..ntp
            })
            .is_err());
    }

    #[test]
    fn one_ntp_mark_stamps_with_the_rate_prior_and_bias() {
        let mut clock = SampleClock::new();
        let mark = Mark {
            source: TimeSource::Ntp,
            sample_s: 100.0,
            utc_ns: T0_NS,
            sigma_s: 1e-6,
            bias_s: 5e-3,
        };
        assert_eq!(clock.add(mark).unwrap(), MarkOutcome::Accepted);
        let st = clock.stamp(110.0).unwrap();
        assert!((5.0e-3..=5.2e-3).contains(&st.sigma_s), "{}", st.sigma_s);
        assert_eq!(st.utc_ns, T0_NS + 10_000_000_000);
        assert!(clock.usable(1e6));
    }

    #[test]
    fn the_set_stamps_with_the_best_usable_source() {
        let mut set = ClockSet::new();
        assert!(set.stamp(0.0, 0.0).is_none());
        let ntp = Mark {
            source: TimeSource::Ntp,
            sample_s: 0.0,
            utc_ns: T0_NS,
            sigma_s: 1e-3,
            bias_s: 0.0,
        };
        set.add(ntp).unwrap();
        set.add(gps(1.0, T0_NS + 1_000_000_000)).unwrap();
        set.add(gps(2.0, T0_NS + 2_000_000_000)).unwrap();
        assert_eq!(set.best(3.0), Some(TimeSource::Gps));
        assert_eq!(set.best(8.0), Some(TimeSource::Ntp));
        let (src, st) = set.stamp(3.0, 1.5).unwrap();
        assert_eq!(src, TimeSource::Gps);
        assert_eq!(st.utc_ns, T0_NS + 1_500_000_000);
        assert!(is_fine(src, st.sigma_s) && !is_fine(TimeSource::Ntp, 1e-6));
        assert_eq!(TimeSource::from_rank(3), Some(TimeSource::CommonView));
        assert_eq!(TimeSource::from_rank(0), None);
        set.reset_source(TimeSource::Gps);
        assert_eq!(set.best(3.0), Some(TimeSource::Ntp));
    }
}
