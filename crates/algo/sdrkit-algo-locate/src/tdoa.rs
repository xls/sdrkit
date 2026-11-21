//! Position of a source from the arrival times of its signal at several receivers (time difference of arrival).
//!
//! The unknowns are latitude, longitude and one time offset per clock group. A clock group is a set of receivers whose clocks
//! share one time scale; the scales of two groups may differ by any constant. An arrival time may carry an ambiguity: a period
//! by whose whole multiples it may be wrong.
//!
//! [`solve`] evaluates a grid around the receivers, descends from the best grid points by Gauss-Newton with Huber weights, drops
//! outliers while enough degrees of freedom remain, computes the 95 % error ellipse and tests the fit by chi-square.

use sdrkit_algo_dsp::linalg::cholesky_solve;
use sdrkit_util_geo::Position;

use crate::propagation::{destination, distance_m, Propagation, SPEED_OF_LIGHT_M_S};
use crate::stats::chi_square_quantile;
use crate::LocateError;

/// Radii of the start grid around the receivers' centroid, m.
const GRID_RADII_M: [f64; 13] = [
    0.0, 30e3, 80e3, 150e3, 250e3, 400e3, 600e3, 900e3, 1300e3, 1800e3, 2500e3, 3200e3, 4000e3,
];
/// Azimuths per grid circle, and the turn of the circles beyond `GRID_TURN_BEYOND_M`, degrees.
const GRID_AZIMUTHS: u32 = 24;
const GRID_TURN_DEG: f64 = 7.5;
const GRID_TURN_BEYOND_M: f64 = 1000e3;
/// The best grid points, at least this far apart, start a descent each.
const STARTS: usize = 5;
const START_SEPARATION_M: f64 = 150e3;
/// The step of the numerical derivatives, north and east, m.
const DERIVATIVE_STEP_M: f64 = 2.0;
/// A residual beyond `HUBER_K` sigmas is down-weighted by `HUBER_K / z`.
const HUBER_K: f64 = 2.5;
/// The longest step of one iteration, m.
const MAX_STEP_M: f64 = 500e3;
/// The latitude of a trial position stays within this, degrees.
const MAX_LAT_DEG: f64 = 89.9;
const MAX_ITERATIONS: usize = 30;
/// A descent stops after a step shorter than this, m.
const STOP_STEP_M: f64 = 0.05;
/// With 2 or more degrees of freedom, the worst residual beyond this many sigmas is dropped.
const OUTLIER_SIGMAS: f64 = 4.0;
/// The semi-axes of the 95 % ellipse are this times the square root of the eigenvalues: sqrt(chi^2(0.95, 2)).
const ELLIPSE_95: f64 = 2.4477;
/// The probability of the chi-square test.
const CHI2_PROBABILITY: f64 = 0.999;
/// A three-receiver fix is ambiguous when another descent ends below this chi^2 farther than the larger of
/// `AMBIGUOUS_MIN_M` and `AMBIGUOUS_AXES` semi-major axes away.
const AMBIGUOUS_CHI2: f64 = 1.0;
const AMBIGUOUS_MIN_M: f64 = 5e3;
const AMBIGUOUS_AXES: f64 = 3.0;
/// The most observations [`solve`] takes.
pub const MAX_OBSERVATIONS: usize = 256;

/// The arrival of the signal at one receiver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Observation {
    /// The receiver's position.
    pub position: Position,
    /// The arrival time on the receiver's clock, s, relative to any origin common to all observations of the call.
    pub time_s: f64,
    /// The 1-sigma of the arrival time, s (above 0).
    pub sigma_s: f64,
    /// The clock group of the receiver.
    pub group: u32,
    /// The period by whose whole multiples the time may be wrong, s; 0 for none.
    pub ambiguity_s: f64,
}

/// The settings of [`solve`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolverConfig {
    /// The propagation model of the travel times.
    pub propagation: Propagation,
    /// The start grid reaches this far from the receivers' centroid, m (above 0). Default 4000 km.
    pub search_limit_m: f64,
    /// Whether a fix without degrees of freedom (three receivers in one group) is allowed. Default off.
    pub three_stations: bool,
}

impl Default for SolverConfig {
    fn default() -> Self {
        SolverConfig {
            propagation: Propagation::default(),
            search_limit_m: 4_000e3,
            three_stations: false,
        }
    }
}

/// What the fix made of one observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    /// The observation is part of the fix.
    Used,
    /// The observation was dropped as an outlier.
    Outlier,
    /// Fewer than 2 observations of its clock group are used, so it constrains nothing and was left out.
    Lone,
}

/// An error ellipse: semi-axes and the azimuth of the major axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ellipse {
    /// The semi-major axis, m.
    pub major_m: f64,
    /// The semi-minor axis, m.
    pub minor_m: f64,
    /// The azimuth of the major axis, degrees from north towards east, 0 to 180.
    pub azimuth_deg: f64,
}

impl Ellipse {
    /// Whether the point `north_m`, `east_m` from the centre lies inside or on the ellipse.
    pub fn contains(&self, north_m: f64, east_m: f64) -> bool {
        let (s, c) = self.azimuth_deg.to_radians().sin_cos();
        let u = (north_m * c + east_m * s) / self.major_m;
        let v = (east_m * c - north_m * s) / self.minor_m;
        u * u + v * v <= 1.0
    }
}

/// The time offset of one clock group.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroupOffset {
    /// The clock group.
    pub group: u32,
    /// The emission time of the signal on that group's clock, s, on the origin of the observation times.
    pub offset_s: f64,
}

/// A position fix.
#[derive(Clone, Debug, PartialEq)]
pub struct Fix {
    /// The position of the source (height 0).
    pub position: Position,
    /// The offset of each clock group used, by group number.
    pub offsets: Vec<GroupOffset>,
    /// `sum (r_i / sigma_i)^2` over the observations used.
    pub chi2: f64,
    /// Degrees of freedom: observations used - 2 - clock groups used.
    pub dof: usize,
    /// The 99.9 % chi-square limit for `dof`; None for 0 degrees of freedom.
    pub chi2_limit: Option<f64>,
    /// The root mean square of the residuals used, s.
    pub rms_s: f64,
    /// The 95 % error ellipse.
    pub ellipse: Ellipse,
    /// What became of each observation, in the order given.
    pub uses: Vec<Use>,
    /// The residual of each observation, s, in the order given; NaN when its clock group is not used.
    pub residuals_s: Vec<f64>,
    /// No degree of freedom checks the fix (three receivers).
    pub unchecked: bool,
    /// A three-receiver fix with a second solution that fits as well.
    pub ambiguous: bool,
}

impl Fix {
    /// The offset of a clock group, if it is used.
    pub fn offset_s(&self, group: u32) -> Option<f64> {
        self.offsets
            .iter()
            .find(|o| o.group == group)
            .map(|o| o.offset_s)
    }

    /// The number of observations used.
    pub fn used(&self) -> usize {
        self.uses.iter().filter(|&&u| u == Use::Used).count()
    }
}

/// Why [`solve`] gives no fix.
#[derive(Clone, Debug, PartialEq)]
pub enum SolveError {
    /// An observation or a setting is out of range.
    Input(LocateError),
    /// Too few observations for the unknowns.
    TooFew,
    /// The best position fails the chi-square test; it is attached.
    ChiSquare(Box<Fix>),
    /// The geometry fixes no position (the normal matrix is singular).
    Degenerate,
}

impl std::fmt::Display for SolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SolveError::Input(e) => write!(f, "{e}"),
            SolveError::TooFew => f.write_str("too few observations"),
            SolveError::ChiSquare(fix) => write!(f, "chi-square {:.1} above the limit", fix.chi2),
            SolveError::Degenerate => f.write_str("the geometry fixes no position"),
        }
    }
}

impl std::error::Error for SolveError {}

impl From<LocateError> for SolveError {
    fn from(e: LocateError) -> Self {
        SolveError::Input(e)
    }
}

/// Locates the source of `observations`.
///
/// Observations of a clock group with fewer than 2 used observations are left out. With fewer than 0 degrees of freedom, or 0
/// when `three_stations` is off, there is no fix. At a trial position each group's anchor is its best-timed observation without
/// ambiguity (else its best-timed); an ambiguous time moves by the whole periods that bring it nearest the anchor; the group
/// offset is the weighted mean of time minus travel time. Five starts from a grid around the receivers descend by Gauss-Newton
/// (numerical derivatives, Huber weights, steps of at most 500 km, at most 30 iterations, stop under 5 cm); the lowest chi^2 wins.
/// With 2 or more degrees of freedom the worst residual beyond 4 sigmas is dropped and the descents repeat. The 95 % ellipse comes
/// from the inverse of the normal matrix, scaled by chi^2 / dof when that exceeds 1. With 1 or more degrees of freedom the fix
/// is refused when chi^2 exceeds the 99.9 % quantile.
///
/// ```
/// use sdrkit_algo_locate::tdoa::{solve, Observation, SolverConfig};
/// use sdrkit_algo_locate::propagation::{destination, Propagation};
/// use sdrkit_util_geo::Position;
/// let centre = Position::new(48.0, 8.0);
/// let source = Position::new(48.3, 9.1);
/// let prop = Propagation::default();
/// let obs: Vec<Observation> = (0..5)
///     .map(|k| {
///         let position = destination(&centre, 72.0 * k as f64, 150e3);
///         let time_s = 0.25 + prop.delay_s(&source, &position);
///         Observation { position, time_s, sigma_s: 1e-6, group: 0, ambiguity_s: 0.0 }
///     })
///     .collect();
/// let fix = solve(&obs, &SolverConfig::default()).unwrap();
/// assert!(fix.position.distance_m(&source) < 10.0);
/// assert!((fix.offset_s(0).unwrap() - 0.25).abs() < 1e-8);
/// ```
pub fn solve(observations: &[Observation], config: &SolverConfig) -> Result<Fix, SolveError> {
    config.propagation.validate()?;
    if !(config.search_limit_m > 0.0 && config.search_limit_m.is_finite()) {
        return Err(LocateError::OutOfRange {
            what: "search limit",
            got: config.search_limit_m,
        }
        .into());
    }
    if observations.len() > MAX_OBSERVATIONS {
        return Err(LocateError::OutOfRange {
            what: "number of observations",
            got: observations.len() as f64,
        }
        .into());
    }
    for o in observations {
        check(o)?;
    }
    let min_dof = if config.three_stations { 0 } else { 1 };
    let mut problem = Problem::new(
        observations,
        config.propagation,
        vec![Use::Used; observations.len()],
    );
    if problem.dof() < min_dof {
        return Err(SolveError::TooFew);
    }
    let starts = problem.starts(config.search_limit_m);
    let mut ends: Vec<(Position, f64)> = starts.iter().map(|s| problem.descend(s)).collect();
    let mut best = lowest(&ends);
    while problem.dof() >= 2 {
        let eval = problem.evaluate(&best.0);
        let worst = (0..observations.len())
            .filter(|&i| problem.uses[i] == Use::Used)
            .map(|i| {
                (
                    i,
                    (problem.residual(&eval, i) / observations[i].sigma_s).abs(),
                )
            })
            .max_by(|a, b| a.1.total_cmp(&b.1));
        let Some((worst, z)) = worst else { break };
        if z.is_nan() || z <= OUTLIER_SIGMAS {
            break;
        }
        let mut uses = problem.uses.clone();
        uses[worst] = Use::Outlier;
        problem = Problem::new(observations, config.propagation, uses);
        if problem.dof() < min_dof {
            return Err(SolveError::TooFew);
        }
        let last = best.0;
        ends = starts
            .iter()
            .chain(std::iter::once(&last))
            .map(|s| problem.descend(s))
            .collect();
        best = lowest(&ends);
    }
    problem.finish(best.0, &ends)
}

fn check(o: &Observation) -> Result<(), LocateError> {
    if !(o.position.lat_deg.abs() <= 90.0 && o.position.lon_deg.is_finite()) {
        return LocateError::range("receiver latitude or longitude", o.position.lat_deg);
    }
    if !o.time_s.is_finite() {
        return LocateError::range("arrival time", o.time_s);
    }
    if !(o.sigma_s > 0.0 && o.sigma_s.is_finite()) {
        return LocateError::range("arrival sigma", o.sigma_s);
    }
    if !(o.ambiguity_s >= 0.0 && o.ambiguity_s.is_finite()) {
        return LocateError::range("ambiguity", o.ambiguity_s);
    }
    Ok(())
}

fn lowest(ends: &[(Position, f64)]) -> (Position, f64) {
    ends.iter()
        .copied()
        .min_by(|a, b| key(a.1).total_cmp(&key(b.1)))
        .unwrap_or((Position::new(0.0, 0.0), f64::INFINITY))
}

/// Sorts NaN after every number.
fn key(chi2: f64) -> f64 {
    if chi2.is_nan() {
        f64::INFINITY
    } else {
        chi2
    }
}

/// The observations, which of them are used, and their clock groups.
struct Problem<'a> {
    obs: &'a [Observation],
    prop: Propagation,
    uses: Vec<Use>,
    /// The group numbers used, ascending.
    groups: Vec<u32>,
    /// The index into `groups` of each observation whose group is used.
    group_of: Vec<Option<usize>>,
    /// The anchor observation of each group.
    anchors: Vec<usize>,
}

/// The state of the observations at one trial position.
struct Eval {
    /// Travel times (NaN where not computed).
    travel: Vec<f64>,
    /// Arrival time, moved by whole periods, minus travel time (NaN where not computed).
    shifted: Vec<f64>,
    /// The offset of each group.
    offsets: Vec<f64>,
    chi2: f64,
}

impl<'a> Problem<'a> {
    /// The problem with the given uses, after leaving out the observations of groups with fewer than 2 used.
    fn new(obs: &'a [Observation], prop: Propagation, mut uses: Vec<Use>) -> Self {
        let used_in = |uses: &[Use], g: u32| {
            (0..obs.len())
                .filter(|&i| uses[i] == Use::Used && obs[i].group == g)
                .count()
        };
        let lone: Vec<usize> = (0..obs.len())
            .filter(|&i| uses[i] == Use::Used && used_in(&uses, obs[i].group) < 2)
            .collect();
        for i in lone {
            uses[i] = Use::Lone;
        }
        let mut groups: Vec<u32> = (0..obs.len())
            .filter(|&i| uses[i] == Use::Used)
            .map(|i| obs[i].group)
            .collect();
        groups.sort_unstable();
        groups.dedup();
        let group_of: Vec<Option<usize>> = obs
            .iter()
            .map(|o| groups.binary_search(&o.group).ok())
            .collect();
        let mut anchors = Vec::with_capacity(groups.len());
        for g in 0..groups.len() {
            let (mut exact, mut any): (Option<usize>, Option<usize>) = (None, None);
            for i in (0..obs.len()).filter(|&i| uses[i] == Use::Used && group_of[i] == Some(g)) {
                let better = |b: Option<usize>| b.is_none_or(|b| obs[i].sigma_s < obs[b].sigma_s);
                if obs[i].ambiguity_s == 0.0 && better(exact) {
                    exact = Some(i);
                }
                if better(any) {
                    any = Some(i);
                }
            }
            anchors.push(exact.or(any).unwrap_or(0));
        }
        Problem {
            obs,
            prop,
            uses,
            groups,
            group_of,
            anchors,
        }
    }

    fn used(&self) -> usize {
        self.uses.iter().filter(|&&u| u == Use::Used).count()
    }

    fn dof(&self) -> isize {
        self.used() as isize - 2 - self.groups.len() as isize
    }

    /// Travel times from `p` to the receivers used (or to all with `all`); NaN elsewhere.
    fn travel(&self, p: &Position, all: bool) -> Vec<f64> {
        self.obs
            .iter()
            .zip(&self.uses)
            .map(|(o, &u)| {
                if all || u == Use::Used {
                    self.prop.delay_s(p, &o.position)
                } else {
                    f64::NAN
                }
            })
            .collect()
    }

    fn evaluate(&self, p: &Position) -> Eval {
        self.evaluate_with(self.travel(p, false))
    }

    /// The group offsets and chi^2 for the travel times `travel`. Observations whose group is used are moved by whole periods
    /// towards their group's anchor; the offsets average the used ones.
    fn evaluate_with(&self, travel: Vec<f64>) -> Eval {
        let mut shifted = vec![f64::NAN; self.obs.len()];
        for (i, o) in self.obs.iter().enumerate() {
            if travel[i].is_finite() {
                shifted[i] = o.time_s - travel[i];
            }
        }
        for (i, o) in self.obs.iter().enumerate() {
            if let Some(g) = self.group_of[i] {
                let a = self.anchors[g];
                if i != a && o.ambiguity_s > 0.0 && shifted[i].is_finite() {
                    let k = ((shifted[a] - shifted[i]) / o.ambiguity_s).round();
                    shifted[i] += k * o.ambiguity_s;
                }
            }
        }
        let mut sw = vec![0.0; self.groups.len()];
        let mut swd = vec![0.0; self.groups.len()];
        for (i, o) in self.obs.iter().enumerate() {
            if let (Use::Used, Some(g)) = (self.uses[i], self.group_of[i]) {
                let w = 1.0 / (o.sigma_s * o.sigma_s);
                sw[g] += w;
                swd[g] += w * shifted[i];
            }
        }
        let offsets: Vec<f64> = swd.iter().zip(&sw).map(|(a, b)| a / b).collect();
        let mut chi2 = 0.0;
        for (i, o) in self.obs.iter().enumerate() {
            if let (Use::Used, Some(g)) = (self.uses[i], self.group_of[i]) {
                let z = (shifted[i] - offsets[g]) / o.sigma_s;
                chi2 += z * z;
            }
        }
        Eval {
            travel,
            shifted,
            offsets,
            chi2,
        }
    }

    fn residual(&self, e: &Eval, i: usize) -> f64 {
        match self.group_of[i] {
            Some(g) => e.shifted[i] - e.offsets[g],
            None => f64::NAN,
        }
    }

    /// The normal equations at `p` in the unknowns (north / c, east / c, one offset per group), all in seconds; `huber` selects
    /// Huber weights (else plain weights). Returns the matrix (row major) and the right-hand side.
    fn normal(&self, p: &Position, e: &Eval, huber: bool) -> (Vec<f64>, Vec<f64>) {
        let north = self.travel(&destination(p, 0.0, DERIVATIVE_STEP_M), false);
        let east = self.travel(&destination(p, 90.0, DERIVATIVE_STEP_M), false);
        let scale = SPEED_OF_LIGHT_M_S / DERIVATIVE_STEP_M;
        let m = 2 + self.groups.len();
        let mut a = vec![0.0; m * m];
        let mut b = vec![0.0; m];
        let mut row = vec![0.0; m];
        for (i, o) in self.obs.iter().enumerate() {
            let (Use::Used, Some(g)) = (self.uses[i], self.group_of[i]) else {
                continue;
            };
            let r = self.residual(e, i);
            let z = (r / o.sigma_s).abs();
            let hw = if huber && z > HUBER_K {
                HUBER_K / z
            } else {
                1.0
            };
            let w = hw / (o.sigma_s * o.sigma_s);
            row.fill(0.0);
            row[0] = (north[i] - e.travel[i]) * scale;
            row[1] = (east[i] - e.travel[i]) * scale;
            row[2 + g] = 1.0;
            for j in 0..m {
                if row[j] == 0.0 {
                    continue;
                }
                for k in 0..m {
                    a[j * m + k] += w * row[j] * row[k];
                }
                b[j] += w * row[j] * r;
            }
        }
        (a, b)
    }

    /// One Gauss-Newton step from `p`: (north, east) in metres. None when the normal matrix is singular.
    fn step(&self, p: &Position) -> Option<(f64, f64)> {
        let e = self.evaluate(p);
        let (mut a, mut b) = self.normal(p, &e, true);
        let m = b.len();
        cholesky_solve(&mut a, &mut b, m).ok()?;
        let (n, east) = (b[0] * SPEED_OF_LIGHT_M_S, b[1] * SPEED_OF_LIGHT_M_S);
        (n.is_finite() && east.is_finite()).then_some((n, east))
    }

    /// Descends from `start`; returns the final position and its chi^2.
    fn descend(&self, start: &Position) -> (Position, f64) {
        let mut p = *start;
        for _ in 0..MAX_ITERATIONS {
            let Some((mut n, mut e)) = self.step(&p) else {
                break;
            };
            let mut len = n.hypot(e);
            if len > MAX_STEP_M {
                n *= MAX_STEP_M / len;
                e *= MAX_STEP_M / len;
                len = MAX_STEP_M;
            }
            p = destination(&p, e.atan2(n).to_degrees(), len);
            p.lat_deg = p.lat_deg.clamp(-MAX_LAT_DEG, MAX_LAT_DEG);
            p.alt_m = 0.0;
            if len < STOP_STEP_M {
                break;
            }
        }
        let chi2 = self.evaluate(&p).chi2;
        (p, chi2)
    }

    /// The starts: the lowest-chi^2 points of a grid of circles around the centroid of the receivers used, at least
    /// `START_SEPARATION_M` apart.
    fn starts(&self, limit_m: f64) -> Vec<Position> {
        let (mut x, mut y, mut z) = (0.0, 0.0, 0.0);
        for (o, &u) in self.obs.iter().zip(&self.uses) {
            if u == Use::Used {
                let (sl, cl) = o.position.lat_deg.to_radians().sin_cos();
                let (so, co) = o.position.lon_deg.to_radians().sin_cos();
                x += cl * co;
                y += cl * so;
                z += sl;
            }
        }
        let norm = (x * x + y * y + z * z).sqrt();
        let centre = if norm > 1e-9 {
            Position::new((z / norm).asin().to_degrees(), y.atan2(x).to_degrees())
        } else {
            self.obs[self.anchors.first().copied().unwrap_or(0)].position
        };
        let mut points = vec![(self.evaluate(&centre).chi2, centre)];
        for &r in GRID_RADII_M[1..].iter().filter(|&&r| r <= limit_m) {
            let turn = if r > GRID_TURN_BEYOND_M {
                GRID_TURN_DEG
            } else {
                0.0
            };
            for k in 0..GRID_AZIMUTHS {
                let q = destination(
                    &centre,
                    f64::from(k) * 360.0 / f64::from(GRID_AZIMUTHS) + turn,
                    r,
                );
                points.push((self.evaluate(&q).chi2, q));
            }
        }
        points.sort_by(|a, b| key(a.0).total_cmp(&key(b.0)));
        let mut starts: Vec<Position> = Vec::with_capacity(STARTS);
        for (_, q) in points {
            if starts
                .iter()
                .all(|s| s.distance_m(&q) >= START_SEPARATION_M)
            {
                starts.push(q);
                if starts.len() == STARTS {
                    break;
                }
            }
        }
        starts
    }

    /// The fix at `best`: offsets, residuals of every observation, the error ellipse and the chi-square test. `ends` are the
    /// end points of the last descents, for the ambiguity of a three-receiver fix.
    fn finish(&self, best: Position, ends: &[(Position, f64)]) -> Result<Fix, SolveError> {
        let e = self.evaluate(&best);
        let dof = self.dof().max(0) as usize;
        // The covariance of north and east: the first two columns of the inverse of the plain normal matrix.
        let (a, _) = self.normal(&best, &e, false);
        let m = 2 + self.groups.len();
        let column = |j: usize| {
            let mut a = a.clone();
            let mut x = vec![0.0; m];
            x[j] = 1.0;
            cholesky_solve(&mut a, &mut x, m).ok().map(|_| x)
        };
        let (Some(cn), Some(ce)) = (column(0), column(1)) else {
            return Err(SolveError::Degenerate);
        };
        let scale = if dof > 0 {
            (e.chi2 / dof as f64).max(1.0)
        } else {
            1.0
        } * SPEED_OF_LIGHT_M_S
            * SPEED_OF_LIGHT_M_S;
        let (cnn, cne, cee) = (cn[0] * scale, cn[1] * scale, ce[1] * scale);
        let half = 0.5 * (cnn + cee);
        let spread = (0.25 * (cnn - cee) * (cnn - cee) + cne * cne).sqrt();
        let ellipse = Ellipse {
            major_m: ELLIPSE_95 * (half + spread).max(0.0).sqrt(),
            minor_m: ELLIPSE_95 * (half - spread).max(0.0).sqrt(),
            azimuth_deg: (0.5 * (2.0 * cne).atan2(cnn - cee))
                .to_degrees()
                .rem_euclid(180.0),
        };
        if !(ellipse.major_m.is_finite() && ellipse.minor_m.is_finite()) {
            return Err(SolveError::Degenerate);
        }
        // Residuals of every observation whose group is used, outliers included.
        let all = self.evaluate_with(self.travel(&best, true));
        let residuals_s: Vec<f64> = (0..self.obs.len())
            .map(|i| self.residual(&all, i))
            .collect();
        let used = self.used();
        let sq: f64 = (0..self.obs.len())
            .filter(|&i| self.uses[i] == Use::Used)
            .map(|i| residuals_s[i] * residuals_s[i])
            .sum();
        let ambiguous = dof == 0
            && ends.iter().any(|(q, c)| {
                *c < AMBIGUOUS_CHI2
                    && distance_m(q, &best) > AMBIGUOUS_MIN_M.max(AMBIGUOUS_AXES * ellipse.major_m)
            });
        let chi2_limit = (dof > 0).then(|| chi_square_quantile(CHI2_PROBABILITY, dof as f64));
        let fix = Fix {
            position: Position::new(best.lat_deg, best.lon_deg),
            offsets: self
                .groups
                .iter()
                .zip(&e.offsets)
                .map(|(&group, &offset_s)| GroupOffset { group, offset_s })
                .collect(),
            chi2: e.chi2,
            dof,
            chi2_limit,
            rms_s: (sq / used.max(1) as f64).sqrt(),
            ellipse,
            uses: self.uses.clone(),
            residuals_s,
            unchecked: dof == 0,
            ambiguous,
        };
        match chi2_limit {
            Some(limit) if fix.chi2.is_nan() || fix.chi2 > limit => {
                Err(SolveError::ChiSquare(Box::new(fix)))
            }
            _ => Ok(fix),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::propagation::north_east_m;
    use crate::testkit::{around, median, observations, ring, uneven_ring};
    use sdrkit_algo_sim::rng::Rng;

    const CENTRE: Position = Position::new(48.0, 8.0);
    const SOURCE: Position = Position::new(48.3, 9.1);

    #[test]
    fn six_receivers_drop_the_late_one() {
        let stations = uneven_ring(&CENTRE, 200e3);
        let mut rng = Rng::new(1);
        let mut obs = observations(
            &mut rng,
            &stations,
            &SOURCE,
            &Propagation::default(),
            500e-9,
        );
        obs[2].time_s += 80e-6;
        let fix = solve(&obs, &SolverConfig::default()).unwrap();
        assert!(
            fix.position.distance_m(&SOURCE) < 500.0,
            "{:?}",
            fix.position
        );
        assert_eq!(fix.uses[2], Use::Outlier);
        assert_eq!(fix.used(), 5);
        assert_eq!(fix.dof, 2);
        assert!(fix.residuals_s[2] > 60e-6, "{:?}", fix.residuals_s);
        assert!(!fix.unchecked && !fix.ambiguous);
    }

    #[test]
    fn two_clock_groups_each_get_an_offset() {
        let stations = uneven_ring(&CENTRE, 200e3);
        let mut rng = Rng::new(2);
        let mut obs = observations(
            &mut rng,
            &stations,
            &SOURCE,
            &Propagation::default(),
            300e-9,
        );
        for o in &mut obs[3..] {
            o.group = 1;
            o.time_s += 3.7e-3;
        }
        let fix = solve(&obs, &SolverConfig::default()).unwrap();
        assert!(
            fix.position.distance_m(&SOURCE) < 500.0,
            "{:?}",
            fix.position
        );
        assert_eq!(fix.dof, 2);
        let gap = fix.offset_s(1).unwrap() - fix.offset_s(0).unwrap();
        assert!((gap - 3.7e-3).abs() < 2e-6, "{gap}");
    }

    #[test]
    fn an_ambiguous_time_moves_by_whole_periods() {
        let stations = uneven_ring(&CENTRE, 200e3);
        let mut rng = Rng::new(3);
        let mut obs = observations(
            &mut rng,
            &stations,
            &SOURCE,
            &Propagation::default(),
            300e-9,
        );
        obs[1].time_s += 5e-3;
        obs[1].ambiguity_s = 5e-3;
        let fix = solve(&obs, &SolverConfig::default()).unwrap();
        assert!(
            fix.position.distance_m(&SOURCE) < 500.0,
            "{:?}",
            fix.position
        );
        assert_eq!(fix.used(), 6);
        assert!(fix.residuals_s[1].abs() < 5e-6);
    }

    #[test]
    fn three_receivers_give_an_unchecked_fix() {
        let stations = uneven_ring(&CENTRE, 200e3);
        let mut rng = Rng::new(4);
        let obs = observations(
            &mut rng,
            &stations[..3],
            &SOURCE,
            &Propagation::default(),
            300e-9,
        );
        let three = SolverConfig {
            three_stations: true,
            ..SolverConfig::default()
        };
        let fix = solve(&obs, &three).unwrap();
        assert!(fix.unchecked && fix.dof == 0 && fix.chi2_limit.is_none());
        assert!(
            fix.position.distance_m(&SOURCE) < 2_000.0,
            "{:?}",
            fix.position
        );
        assert_eq!(
            solve(&obs, &SolverConfig::default()),
            Err(SolveError::TooFew)
        );
    }

    #[test]
    fn three_receivers_can_have_two_solutions() {
        // Three receivers on a 150 km circle; sources up to 600 km away, where two positions can fit the same times.
        let stations = ring(&CENTRE, 150e3, 3);
        let prop = Propagation::default();
        let three = SolverConfig {
            three_stations: true,
            ..SolverConfig::default()
        };
        let mut rng = Rng::new(10);
        let (mut solved, mut ambiguous) = (0, 0);
        for _ in 0..40 {
            let source = around(&mut rng, &CENTRE, 600e3);
            let obs = observations(&mut rng, &stations, &source, &prop, 100e-9);
            if let Ok(fix) = solve(&obs, &three) {
                solved += 1;
                ambiguous += usize::from(fix.ambiguous);
            }
        }
        assert!(
            ambiguous >= 1 && ambiguous < solved,
            "{ambiguous} of {solved}"
        );
        let obs = observations(&mut rng, &stations, &CENTRE, &prop, 100e-9);
        let fix = solve(&obs, &three).unwrap();
        assert!(!fix.ambiguous && fix.position.distance_m(&CENTRE) < 500.0);
    }

    #[test]
    fn a_second_source_fails_the_chi_square_test() {
        let stations = uneven_ring(&CENTRE, 200e3);
        let prop = Propagation::default();
        let mut rng = Rng::new(5);
        let mut obs = observations(&mut rng, &stations[..4], &SOURCE, &prop, 300e-9);
        let other = Position::new(49.5, 6.0);
        for o in &mut obs[2..] {
            o.time_s = prop.delay_s(&other, &o.position);
        }
        match solve(&obs, &SolverConfig::default()) {
            Err(SolveError::ChiSquare(fix)) => assert!(fix.chi2 > fix.chi2_limit.unwrap()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bad_input_is_refused() {
        let stations = ring(&CENTRE, 150e3, 4);
        let mut rng = Rng::new(6);
        let obs = observations(&mut rng, &stations, &SOURCE, &Propagation::default(), 1e-6);
        let cfg = SolverConfig::default();
        let mut bad = obs.clone();
        bad[0].sigma_s = 0.0;
        assert!(matches!(solve(&bad, &cfg), Err(SolveError::Input(_))));
        bad[0].sigma_s = 1e-6;
        bad[1].time_s = f64::NAN;
        assert!(matches!(solve(&bad, &cfg), Err(SolveError::Input(_))));
        assert_eq!(solve(&obs[..2], &cfg), Err(SolveError::TooFew));
        // A receiver alone in its group carries only its own offset.
        let mut lone = obs.clone();
        lone[3].group = 9;
        assert_eq!(solve(&lone, &cfg), Err(SolveError::TooFew));
        let fix = solve(
            &lone,
            &SolverConfig {
                three_stations: true,
                ..cfg
            },
        )
        .unwrap();
        assert_eq!(fix.uses[3], Use::Lone);
        assert!(fix.residuals_s[3].is_nan());
    }

    /// Position errors (m) of `trials` sources within `spread_m` of the centre of `stations`, and the share solved.
    fn trial_errors(
        seed: u64,
        stations: &[Position],
        spread_m: f64,
        sigma_s: f64,
        truth: &Propagation,
        model: &Propagation,
        trials: usize,
    ) -> (Vec<f64>, Vec<bool>, f64) {
        let mut rng = Rng::new(seed);
        let cfg = SolverConfig {
            propagation: *model,
            ..SolverConfig::default()
        };
        let (mut errors, mut inside, mut solved) = (Vec::new(), Vec::new(), 0usize);
        for _ in 0..trials {
            let source = around(&mut rng, &CENTRE, spread_m);
            let obs = observations(&mut rng, stations, &source, truth, sigma_s);
            if let Ok(fix) = solve(&obs, &cfg) {
                solved += 1;
                errors.push(distance_m(&fix.position, &source));
                let (n, e) = north_east_m(&fix.position, &source);
                inside.push(fix.ellipse.contains(n, e));
            }
        }
        (errors, inside, solved as f64 / trials as f64)
    }

    #[test]
    fn accuracy_follows_the_timing_error() {
        // 6 receivers on a 150 km circle, sources within 300 km, 200 trials per level.
        let stations = ring(&CENTRE, 150e3, 6);
        let prop = Propagation::default();
        for (k, sigma) in [0.1e-6, 0.3e-6, 1e-6, 3e-6, 10e-6].into_iter().enumerate() {
            let (errors, _, solved) =
                trial_errors(100 + k as u64, &stations, 300e3, sigma, &prop, &prop, 200);
            let med = median(&errors);
            assert!(solved >= 0.97, "{sigma}: solved {solved}");
            assert!(
                med < 2.0 * SPEED_OF_LIGHT_M_S * sigma,
                "{sigma}: median {med}"
            );
        }
    }

    #[test]
    fn the_95_percent_ellipse_covers_the_errors() {
        let stations = ring(&CENTRE, 150e3, 6);
        let prop = Propagation::default();
        let (_, inside, _) = trial_errors(200, &stations, 300e3, 1e-6, &prop, &prop, 200);
        let share = inside.iter().filter(|&&x| x).count() as f64 / inside.len() as f64;
        assert!((0.85..=1.0).contains(&share), "{share}");
    }

    #[test]
    fn long_paths_need_the_hop_model() {
        // 6 receivers on a 1500 km circle, sources within 2500 km, 1 us; 60 trials per model.
        let stations = ring(&CENTRE, 1500e3, 6);
        let hops = Propagation {
            hops: true,
            ..Propagation::default()
        };
        let ground = Propagation::default();
        let (errors, _, solved) = trial_errors(300, &stations, 2500e3, 1e-6, &hops, &hops, 60);
        let hop_median = median(&errors);
        assert!(
            solved >= 0.9 && hop_median < 2_000.0,
            "hops: solved {solved}, median {hop_median}"
        );
        let (errors, _, solved) = trial_errors(300, &stations, 2500e3, 1e-6, &hops, &ground, 60);
        let ground_median = if errors.is_empty() {
            f64::INFINITY
        } else {
            median(&errors)
        };
        assert!(
            solved < 0.5 || ground_median > 5.0 * hop_median,
            "ground: solved {solved}, median {ground_median}"
        );
    }
}
