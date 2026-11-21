//! Matching of detections from several receivers into sets that one source explains, and their positions.
//!
//! Detections wait a while in time order, so that late network packets still arrive. The oldest becomes the seed. From every
//! other receiver, the detections that one source could have caused together with the seed are candidates, nearest in time first.
//! The seed and one candidate per receiver form a group; fine detections of one clock group that contradict one another are
//! pruned, and the fine rest goes to [`crate::tdoa::solve`]. When that fails, the receiver with the worst residual moves on to its
//! next candidate. Coarse detections that agree with a fix are reported as also heard.
//!
//! Clock group [`UTC_GROUP`] is UTC; a fix without it has a time on another scale.

use sdrkit_util_geo::Position;

use crate::propagation::{distance_m, Propagation, SPEED_OF_LIGHT_M_S};
use crate::tdoa::{solve, Fix, Observation, SolveError, SolverConfig, Use};
use crate::LocateError;

/// The clock group of UTC.
pub const UTC_GROUP: u32 = 0;

/// The candidate bound assumes paths no faster than this fraction of the speed of light.
const BOUND_SPEED: f64 = 0.98;
/// The margin, sigma multiple and ambiguity multiple of the candidate bound.
const BOUND_MARGIN_S: f64 = 2e-6;
const BOUND_SIGMAS: f64 = 5.0;
const BOUND_AMBIGUITIES: f64 = 3.0;
/// The pulse widths of one group lie within this factor of the seed's.
const WIDTH_FACTOR: f64 = 8.0;
/// Solves per seed.
const MAX_ATTEMPTS: usize = 8;
/// A coarse detection is also heard within this many sigmas plus the margin of the arrival time that the fix predicts.
const HEARD_SIGMAS: f64 = 5.0;
const HEARD_MARGIN_S: f64 = 2e-6;

/// One receiver's detection of a signal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    /// The receiver: an index into the position list given to [`Matcher::run`].
    pub station: usize,
    /// The arrival time on the receiver's clock group, ns.
    pub time_ns: i64,
    /// The 1-sigma of the time, s (above 0).
    pub sigma_s: f64,
    /// The clock group ([`UTC_GROUP`] for UTC).
    pub group: u32,
    /// The period by whose whole multiples the time may be wrong, s; 0 for none.
    pub ambiguity_s: f64,
    /// The pulse width, s; 0 when unknown (no width check).
    pub width_s: f64,
    /// Whether the time is good enough to take part in a fix.
    pub fine: bool,
}

/// The settings of a [`Matcher`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatcherConfig {
    /// The propagation model.
    pub propagation: Propagation,
    /// The start grid of the solver reaches this far, m. Default 4000 km.
    pub search_limit_m: f64,
    /// Fine receivers that a fix needs, 3 to 64; three-receiver fixes (no degree of freedom) only at 3. Default 4.
    pub min_stations: usize,
    /// How long a detection waits before it can be a seed, s. Default 2.
    pub wait_s: f64,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        MatcherConfig {
            propagation: Propagation::default(),
            search_limit_m: 4_000e3,
            min_stations: 4,
            wait_s: 2.0,
        }
    }
}

/// A located source.
#[derive(Clone, Debug, PartialEq)]
pub struct Located {
    /// The emission time, ns: the seed's time plus the offset of [`UTC_GROUP`], or of the first group when it is absent.
    pub time_ns: i64,
    /// Whether `time_ns` is on UTC ([`UTC_GROUP`] took part).
    pub utc: bool,
    /// The fix.
    pub fix: Fix,
    /// The fine detections given to the solver, in the order of `fix.uses` and `fix.residuals_s`.
    pub detections: Vec<Detection>,
    /// Coarse detections from other receivers that agree with the fix.
    pub also_heard: Vec<Detection>,
}

/// Counters of a [`Matcher`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchCounters {
    /// Seeds handled.
    pub seeds: u64,
    /// Fixes made.
    pub fixes: u64,
    /// Solves refused by the chi-square test.
    pub refused: u64,
    /// Seeds dropped after their attempts.
    pub unmatched: u64,
    /// Seeds whose group had enough receivers, but too few fine ones.
    pub coarse_only: u64,
    /// Seeds whose group had too few receivers.
    pub too_few: u64,
}

/// Groups detections from several receivers and locates each group.
#[derive(Clone, Debug)]
pub struct Matcher {
    config: MatcherConfig,
    queue: Vec<Detection>,
    counters: MatchCounters,
}

/// A member of a group: its index in the queue, and its receiver's index in the candidate lists (None for the seed).
type Member = (usize, Option<usize>);

impl Matcher {
    /// A matcher. Errors for a setting out of range.
    pub fn new(config: MatcherConfig) -> Result<Self, LocateError> {
        config.propagation.validate()?;
        if !(config.search_limit_m > 0.0 && config.search_limit_m.is_finite()) {
            return LocateError::range("search limit", config.search_limit_m);
        }
        if !(3..=64).contains(&config.min_stations) {
            return LocateError::range("minimum stations", config.min_stations as f64);
        }
        if !(config.wait_s >= 0.0 && config.wait_s <= 3600.0) {
            return LocateError::range("wait", config.wait_s);
        }
        Ok(Matcher {
            config,
            queue: Vec::new(),
            counters: MatchCounters::default(),
        })
    }

    /// The settings.
    pub fn config(&self) -> &MatcherConfig {
        &self.config
    }

    /// Detections waiting.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// The counters.
    pub fn counters(&self) -> MatchCounters {
        self.counters
    }

    /// Forgets the waiting detections and the counters.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.counters = MatchCounters::default();
    }

    /// Adds a detection in time order. Errors for a sigma that is not above 0, or an ambiguity or width that is negative or not
    /// finite.
    pub fn push(&mut self, d: Detection) -> Result<(), LocateError> {
        if !(d.sigma_s > 0.0 && d.sigma_s.is_finite()) {
            return LocateError::range("detection sigma", d.sigma_s);
        }
        if !(d.ambiguity_s >= 0.0 && d.ambiguity_s.is_finite()) {
            return LocateError::range("detection ambiguity", d.ambiguity_s);
        }
        if !(d.width_s >= 0.0 && d.width_s.is_finite()) {
            return LocateError::range("detection width", d.width_s);
        }
        let at = self.queue.partition_point(|q| q.time_ns <= d.time_ns);
        self.queue.insert(at, d);
        Ok(())
    }

    /// Handles every detection older than `now_ns` minus the wait as a seed, oldest first, and appends the sources located.
    /// `stations` holds the receiver positions by index; a detection of an unknown receiver is dropped.
    pub fn run(&mut self, now_ns: i64, stations: &[Position], out: &mut Vec<Located>) {
        let wait_ns = (self.config.wait_s * 1e9).round() as i64;
        let limit = now_ns.saturating_sub(wait_ns);
        while self.queue.first().is_some_and(|d| d.time_ns <= limit) {
            let mut gone = self.seed(stations, out);
            gone.push(0);
            gone.sort_unstable();
            gone.dedup();
            for j in gone.into_iter().rev() {
                self.queue.remove(j);
            }
        }
    }

    /// Handles the oldest detection as a seed; returns the queue indices that leave (the seed always leaves as well).
    fn seed(&mut self, stations: &[Position], out: &mut Vec<Located>) -> Vec<usize> {
        let seed = self.queue[0];
        self.counters.seeds += 1;
        if seed.station >= stations.len() {
            self.counters.unmatched += 1;
            return Vec::new();
        }
        // Candidates per receiver, nearest in time first.
        let mut cands: Vec<(usize, Vec<usize>)> = Vec::new();
        for (j, d) in self.queue.iter().enumerate().skip(1) {
            if d.station == seed.station || d.station >= stations.len() || !widths_agree(&seed, d) {
                continue;
            }
            if !within(
                &seed,
                d,
                distance_m(&stations[seed.station], &stations[d.station]),
            ) {
                continue;
            }
            match cands.iter_mut().find(|c| c.0 == d.station) {
                Some(c) => c.1.push(j),
                None => cands.push((d.station, vec![j])),
            }
        }
        for (_, list) in &mut cands {
            list.sort_by_key(|&j| self.queue[j].time_ns.abs_diff(seed.time_ns));
        }
        let need = self.config.min_stations.max(3);
        let solver = SolverConfig {
            propagation: self.config.propagation,
            search_limit_m: self.config.search_limit_m,
            three_stations: self.config.min_stations == 3,
        };
        let mut next = vec![0usize; cands.len()];
        for attempt in 0..MAX_ATTEMPTS {
            let members: Vec<Member> = std::iter::once((0, None))
                .chain(
                    cands
                        .iter()
                        .enumerate()
                        .filter_map(|(c, (_, list))| list.get(next[c]).map(|&j| (j, Some(c)))),
                )
                .collect();
            let mut fine: Vec<Member> = members
                .iter()
                .copied()
                .filter(|&(j, _)| self.queue[j].fine)
                .collect();
            self.prune(&mut fine, stations);
            if fine.len() < need {
                if attempt > 0 {
                    self.counters.unmatched += 1;
                } else if members.len() >= need {
                    self.counters.coarse_only += 1;
                } else {
                    self.counters.too_few += 1;
                }
                return Vec::new();
            }
            let obs: Vec<Observation> = fine
                .iter()
                .map(|&(j, _)| {
                    let d = &self.queue[j];
                    Observation {
                        position: stations[d.station],
                        time_s: d.time_ns.saturating_sub(seed.time_ns) as f64 * 1e-9,
                        sigma_s: d.sigma_s,
                        group: d.group,
                        ambiguity_s: d.ambiguity_s,
                    }
                })
                .collect();
            let fix = match solve(&obs, &solver) {
                Ok(fix) if fix.used() >= need => {
                    self.counters.fixes += 1;
                    return self.accept(&seed, fix, &fine, &cands, stations, out);
                }
                Ok(fix) => fix,
                Err(SolveError::ChiSquare(fix)) => {
                    self.counters.refused += 1;
                    *fix
                }
                Err(_) => break,
            };
            // The receiver with the worst residual (never the seed) moves on to its next candidate.
            let worst = fine
                .iter()
                .zip(&obs)
                .zip(&fix.residuals_s)
                .filter_map(|((&(_, c), o), r)| c.map(|c| (c, (r / o.sigma_s).abs())))
                .filter(|(_, z)| z.is_finite())
                .max_by(|a, b| a.1.total_cmp(&b.1));
            match worst {
                Some((c, _)) => next[c] += 1,
                None => break,
            }
        }
        self.counters.unmatched += 1;
        Vec::new()
    }

    /// Drops the fine member in most conflicts (never the seed; ties: the farthest in time from the seed) until no two members of
    /// one clock group break the candidate bound.
    fn prune(&self, fine: &mut Vec<Member>, stations: &[Position]) {
        let seed_ns = self.queue[0].time_ns;
        loop {
            let n = fine.len();
            let mut conflicts = vec![0usize; n];
            for a in 0..n {
                for b in a + 1..n {
                    let (da, db) = (&self.queue[fine[a].0], &self.queue[fine[b].0]);
                    if da.group == db.group
                        && !within(
                            da,
                            db,
                            distance_m(&stations[da.station], &stations[db.station]),
                        )
                    {
                        conflicts[a] += 1;
                        conflicts[b] += 1;
                    }
                }
            }
            let pick = (0..n)
                .filter(|&k| fine[k].0 != 0 && conflicts[k] > 0)
                .max_by_key(|&k| {
                    (
                        conflicts[k],
                        self.queue[fine[k].0].time_ns.abs_diff(seed_ns),
                    )
                });
            match pick {
                Some(k) => {
                    fine.remove(k);
                }
                None => return,
            }
        }
    }

    /// Reports a fix; returns the queue indices of the detections used and also heard.
    fn accept(
        &self,
        seed: &Detection,
        fix: Fix,
        fine: &[Member],
        cands: &[(usize, Vec<usize>)],
        stations: &[Position],
        out: &mut Vec<Located>,
    ) -> Vec<usize> {
        let (offset, utc) = match fix.offset_s(UTC_GROUP) {
            Some(o) => (o, true),
            None => (fix.offsets.first().map_or(0.0, |o| o.offset_s), false),
        };
        let mut gone: Vec<usize> = fine
            .iter()
            .zip(&fix.uses)
            .filter(|(_, &u)| u == Use::Used)
            .map(|(&(j, _), _)| j)
            .collect();
        let used_stations: Vec<usize> = gone.iter().map(|&j| self.queue[j].station).collect();
        // How far a coarse detection lies from the arrival the fix predicts, in units of its tolerance.
        let miss = |d: &Detection| {
            let o = fix.offset_s(d.group)?;
            let predicted = o + self
                .config
                .propagation
                .delay_s(&fix.position, &stations[d.station]);
            let off = (d.time_ns.saturating_sub(seed.time_ns) as f64 * 1e-9 - predicted).abs();
            let tolerance = HEARD_SIGMAS * d.sigma_s + HEARD_MARGIN_S;
            (off <= tolerance).then_some(off / tolerance)
        };
        let mut heard: Vec<usize> = Vec::new();
        if !seed.fine && !used_stations.contains(&seed.station) && miss(seed).is_some() {
            heard.push(0);
        }
        for (station, list) in cands {
            if used_stations.contains(station)
                || heard.iter().any(|&j| self.queue[j].station == *station)
            {
                continue;
            }
            let best = list
                .iter()
                .filter(|&&j| !self.queue[j].fine)
                .filter_map(|&j| miss(&self.queue[j]).map(|m| (j, m)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((j, _)) = best {
                heard.push(j);
            }
        }
        out.push(Located {
            time_ns: seed.time_ns.saturating_add((offset * 1e9).round() as i64),
            utc,
            detections: fine.iter().map(|&(j, _)| self.queue[j]).collect(),
            also_heard: heard.iter().map(|&j| self.queue[j]).collect(),
            fix,
        });
        gone.extend(heard);
        gone
    }
}

/// Whether two detections can come from one source: `|dt| <= baseline / (0.98 c) + 2 us + 5 sqrt(s_a^2 + s_b^2) + 3 max(ambiguity)`.
fn within(a: &Detection, b: &Detection, baseline_m: f64) -> bool {
    let dt = a.time_ns.abs_diff(b.time_ns) as f64 * 1e-9;
    let bound = baseline_m / (BOUND_SPEED * SPEED_OF_LIGHT_M_S)
        + BOUND_MARGIN_S
        + BOUND_SIGMAS * (a.sigma_s * a.sigma_s + b.sigma_s * b.sigma_s).sqrt()
        + BOUND_AMBIGUITIES * a.ambiguity_s.max(b.ambiguity_s);
    dt <= bound
}

/// Whether two pulse widths lie within [`WIDTH_FACTOR`] of each other (true when either is unknown).
fn widths_agree(a: &Detection, b: &Detection) -> bool {
    if a.width_s > 0.0 && b.width_s > 0.0 {
        a.width_s.max(b.width_s) <= WIDTH_FACTOR * a.width_s.min(b.width_s)
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{around, ring};
    use sdrkit_algo_sim::rng::Rng;

    const T0_NS: i64 = 1_700_000_000_000_000_000;
    const CENTRE: Position = Position::new(48.0, 8.0);

    fn detection(station: usize, time_ns: i64, sigma_s: f64) -> Detection {
        Detection {
            station,
            time_ns,
            sigma_s,
            group: UTC_GROUP,
            ambiguity_s: 0.0,
            width_s: 100e-6,
            fine: true,
        }
    }

    /// The detections of a source sent at `t_ns` from `source` by every receiver that does not miss it.
    fn detections_of(
        rng: &mut Rng,
        stations: &[Position],
        source: &Position,
        t_ns: i64,
        sigma_s: f64,
        miss: f64,
    ) -> Vec<Detection> {
        let prop = Propagation::default();
        let mut v = Vec::new();
        for (s, q) in stations.iter().enumerate() {
            if rng.uniform() < miss {
                continue;
            }
            let delay = prop.delay_s(source, q) + sigma_s * rng.normal();
            v.push(Detection {
                width_s: rng.range(50e-6, 150e-6),
                ..detection(s, t_ns + (delay * 1e9).round() as i64, sigma_s)
            });
        }
        v
    }

    #[test]
    fn detections_of_many_sources_become_fixes() {
        // 150 sources (every tenth followed by another 2 ms later) within 300 km of 6 receivers on a 150 km circle; each
        // receiver misses 10 %; 84 false detections; 0.3 us timing. A fix is right within 20 us and 3 km of a source.
        let stations = ring(&CENTRE, 150e3, 6);
        let mut rng = Rng::new(8);
        let sigma = 0.3e-6;
        let mut truth: Vec<(i64, Position)> = Vec::new();
        let mut t = T0_NS;
        while truth.len() < 150 {
            t += 2_000_000 + (rng.exponential() * 0.5e9) as i64;
            truth.push((t, around(&mut rng, &CENTRE, 300e3)));
            if truth.len().is_multiple_of(10) && truth.len() < 150 {
                t += 2_000_000;
                truth.push((t, around(&mut rng, &CENTRE, 300e3)));
            }
        }
        let mut dets = Vec::new();
        for (t, p) in &truth {
            dets.extend(detections_of(&mut rng, &stations, p, *t, sigma, 0.1));
        }
        let end = t;
        for _ in 0..84 {
            let at = T0_NS + (rng.uniform() * (end - T0_NS) as f64) as i64;
            dets.push(Detection {
                width_s: rng.range(50e-6, 150e-6),
                ..detection(rng.below(6) as usize, at, sigma)
            });
        }
        // Arrival order differs from time order.
        for k in (1..dets.len()).rev() {
            dets.swap(k, rng.below(k as u64 + 1) as usize);
        }
        let mut m = Matcher::new(MatcherConfig::default()).unwrap();
        for d in dets {
            m.push(d).unwrap();
        }
        let mut out = Vec::new();
        let mut now = T0_NS;
        while now < end + 3_000_000_000 {
            now += 200_000_000;
            m.run(now, &stations, &mut out);
        }
        assert_eq!(m.pending(), 0);
        let right = |l: &Located, (t, p): &(i64, Position)| {
            (l.time_ns - t).abs() <= 20_000 && distance_m(&l.fix.position, p) <= 3_000.0
        };
        let located = truth
            .iter()
            .filter(|s| out.iter().any(|l| right(l, s)))
            .count();
        let wrong = out
            .iter()
            .filter(|l| !truth.iter().any(|s| right(l, s)))
            .count();
        let c = m.counters();
        assert!(
            located >= 128,
            "located {located} of 150, {wrong} wrong of {}, {c:?}",
            out.len()
        );
        assert!(
            wrong as f64 <= 0.01 * out.len() as f64,
            "{wrong} wrong of {}, {c:?}",
            out.len()
        );
        assert!(out.iter().all(|l| l.utc && l.also_heard.is_empty()));
    }

    #[test]
    fn coarse_detections_are_heard_but_never_used() {
        let stations = ring(&CENTRE, 150e3, 6);
        let mut rng = Rng::new(9);
        let source = Position::new(48.4, 8.3);
        let t = T0_NS + 1_000_000_000;
        let mut m = Matcher::new(MatcherConfig::default()).unwrap();
        let mut dets = detections_of(&mut rng, &stations, &source, t, 0.3e-6, 0.0);
        // The last receiver keeps time by NTP: 5 ms coarse, 1 ms off.
        let last = dets.last_mut().unwrap();
        last.time_ns += 1_000_000;
        last.sigma_s = 5e-3;
        last.fine = false;
        for d in dets {
            m.push(d).unwrap();
        }
        let mut out = Vec::new();
        m.run(t + 1_000_000_000, &stations, &mut out);
        assert!(out.is_empty() && m.pending() == 6);
        m.run(t + 3_000_000_000, &stations, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(m.pending(), 0);
        let l = &out[0];
        assert!(distance_m(&l.fix.position, &source) < 1_000.0);
        assert!((l.time_ns - t).abs() < 5_000, "{}", l.time_ns - t);
        assert_eq!((l.detections.len(), l.also_heard.len()), (5, 1));
        assert_eq!(l.also_heard[0].station, 5);
        // Three fine and three coarse receivers: coarse only. Two receivers: too few. Each detection is a seed in turn.
        let mut dets = detections_of(
            &mut rng,
            &stations,
            &source,
            t + 10_000_000_000,
            0.3e-6,
            0.0,
        );
        for d in &mut dets[3..] {
            d.fine = false;
            d.sigma_s = 5e-3;
        }
        dets.extend(detections_of(
            &mut rng,
            &stations[..2],
            &source,
            t + 20_000_000_000,
            0.3e-6,
            0.0,
        ));
        for d in dets {
            m.push(d).unwrap();
        }
        m.run(t + 30_000_000_000, &stations, &mut out);
        let c = m.counters();
        assert_eq!((c.fixes, out.len()), (1, 1));
        assert!(c.coarse_only >= 1 && c.too_few >= 2, "{c:?}");
        assert_eq!(m.pending(), 0);
        m.clear();
        assert_eq!(m.counters(), MatchCounters::default());
        assert!(m.push(detection(0, t, 0.0)).is_err());
        assert!(Matcher::new(MatcherConfig {
            min_stations: 2,
            ..MatcherConfig::default()
        })
        .is_err());
    }
}
