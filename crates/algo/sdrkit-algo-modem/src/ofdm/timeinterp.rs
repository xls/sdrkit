//! The channel at the scattered-pilot grid interpolated in time, with an averaging gain chosen from the measured noise and
//! rate of change of the channel, and the delay window of the impulse response (DVB-T, ISDB-T).

use crate::{Complex32, ModemError, Param};

/// Values kept per grid point.
pub const POINT_HISTORY: usize = 5;

/// The options of a [`PilotTimeInterp`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PilotTimeConfig {
    /// Grid points (carriers that carry a scattered pilot in some symbol).
    pub points: usize,
    /// Symbols between two refreshes of a point (4 for DVB-T and ISDB-T).
    pub refresh: i64,
    /// Symbols the equaliser works behind the newest (8).
    pub delay: usize,
    /// The smoothing of the statistics (0.08).
    pub smoothing: f64,
    /// Statistics needed before the gain search runs (8); the gain is 1 before.
    pub min_stats: u32,
    /// The weight of the change term in the gain search: 1 for statistics taken at the refresh spacing, 16 for statistics of
    /// continual pilots taken one and two symbols apart.
    pub change_weight: f64,
    /// The smallest averaging gain (0.1).
    pub min_gain: f64,
    /// The count of a point's average stops here (1000).
    pub count_cap: u32,
    /// A point not refreshed for more than this many symbols starts again (8).
    pub restart_after: i64,
    /// The noise of a pilot to the noise of a data cell, as power (16/9: the pilots are boosted by 4/3).
    pub pilot_boost: f64,
    /// Time interpolation; off, a point's channel is its newest average.
    pub interpolate: bool,
    /// The statistics come from the scattered pilots themselves (off: the caller gives them, [`PilotTimeInterp::add_statistics`]).
    pub scattered_statistics: bool,
}

impl PilotTimeConfig {
    /// The usual numbers for a grid of `points` refreshed every 4 symbols.
    pub fn new(points: usize) -> Self {
        PilotTimeConfig {
            points,
            refresh: 4,
            delay: 8,
            smoothing: 0.08,
            min_stats: 8,
            change_weight: 1.0,
            min_gain: 0.1,
            count_cap: 1000,
            restart_after: 8,
            pilot_boost: 16.0 / 9.0,
            interpolate: true,
            scattered_statistics: true,
        }
    }
}

/// One grid point.
#[derive(Clone, Copy, Debug, Default)]
struct Point {
    value: Complex32,
    count: u32,
    last: i64,
    /// The raw pilots one and two refreshes back, with their symbols.
    raw1: Option<(i64, Complex32)>,
    raw2: Option<(i64, Complex32)>,
    hist: [(i64, Complex32); POINT_HISTORY],
    len: usize,
}

impl Point {
    fn store(&mut self, t: i64, v: Complex32) {
        if self.len == POINT_HISTORY {
            self.hist.copy_within(1.., 0);
            self.len -= 1;
        }
        self.hist[self.len] = (t, v);
        self.len += 1;
    }

    /// The channel at symbol `t` from the stored averages.
    fn at(&self, t: i64, interpolate: bool) -> Option<Complex32> {
        let h = &self.hist[..self.len];
        let last = h.last()?;
        if !interpolate {
            return Some(last.1);
        }
        let nb = h.iter().take_while(|x| x.0 <= t).count();
        let na = h.len() - nb;
        let pts: &[(i64, Complex32)] = match (nb, na) {
            (_, 0) => return Some(last.1),
            (0, _) => return Some(h[0].1),
            (1, _) | (_, 1) => &h[nb - 1..nb + 1],
            _ => &h[nb - 2..nb + 2],
        };
        let x = t as f64;
        let mut acc = Complex32::default();
        for (i, pi) in pts.iter().enumerate() {
            let mut w = 1.0;
            for (j, pj) in pts.iter().enumerate() {
                if i != j {
                    w *= (x - pj.0 as f64) / (pi.0 - pj.0) as f64;
                }
            }
            acc += pi.1 * w as f32;
        }
        Some(acc)
    }
}

/// The channel of the scattered-pilot grid interpolated in time.
///
/// Per grid point, for each raw pilot `p = Y / pilot value`:
///
/// 1. the two previous raw pilots of the point are kept;
/// 2. over the points refreshed in a symbol, `d1 = mean |p - p(-1)|^2` and `d2 = mean |p - p(-2)|^2` (one and two refreshes
///    back), smoothed (the first value sets them, then `x += smoothing (new - x)`). For a smoothly changing channel `d1 = 2r + q`
///    and `d2 = 2r + 4q`, so the noise of one pilot is `r = (4 d1 - d2) / 6` (at least 1e-12) and the change of the channel
///    between refreshes `q = max(0, (d2 - d1) / 3)`;
/// 3. after `min_stats` statistics the averaging gain is the `a` in {0.05, 0.10, ..., 1.00} that minimises
///    `w q ((1 - a) / a)^2 + r a / (2 - a)` (`w` the change weight), at least `min_gain`; 1 before;
/// 4. the point's average `g += max(a, 1 / count) (p - g)` (count capped) is stored with its symbol, the newest 5 kept; a point
///    not refreshed for more than `restart_after` symbols starts again from its pilot;
/// 5. the channel of a point at symbol `m` ([`channel_at`](Self::channel_at), `m` at least `delay` symbols behind the newest)
///    is the cubic through the two stored values at or before `m` and the two after, a line when one side has only one, the
///    newest when none follows.
///
/// The noise for the soft bits is `r x pilot_boost`, smoothed `0.9 / 0.1` ([`soft_noise`](Self::soft_noise); the caller
/// multiplies it by its calibration). The common phase and timing tracking stay with the caller and compare each symbol with
/// the raw pilots one refresh back, not with this delayed estimate. Nothing is allocated after [`PilotTimeInterp::new`].
#[derive(Clone, Debug)]
pub struct PilotTimeInterp {
    cfg: PilotTimeConfig,
    points: Vec<Point>,
    d1: f64,
    d2: f64,
    have_d1: bool,
    have_d2: bool,
    stats: u32,
    gain: f64,
    soft_noise: Option<f64>,
    newest: Option<i64>,
}

impl PilotTimeInterp {
    /// An estimator with these options. Errors for no points, a refresh under 1, a smoothing outside `(0, 1]`, a change weight
    /// that is negative, or a smallest gain outside `(0, 1]`.
    pub fn new(cfg: PilotTimeConfig) -> Result<Self, ModemError> {
        Param::Points.check_count(cfg.points, 1, 1 << 20)?;
        if cfg.refresh < 1 {
            return Param::Period.at_least(1.0, cfg.refresh as f64);
        }
        Param::Smoothing.check_in(cfg.smoothing, f64::MIN_POSITIVE, 1.0)?;
        Param::Gain.check_in(cfg.change_weight, 0.0, f64::MAX)?;
        Param::Gain.check_in(cfg.min_gain, f64::MIN_POSITIVE, 1.0)?;
        Ok(PilotTimeInterp {
            cfg,
            points: vec![Point::default(); cfg.points],
            d1: 0.0,
            d2: 0.0,
            have_d1: false,
            have_d2: false,
            stats: 0,
            gain: 1.0,
            soft_noise: None,
            newest: None,
        })
    }

    /// The options.
    pub fn config(&self) -> &PilotTimeConfig {
        &self.cfg
    }

    /// Forgets everything (a new acquisition or new parameters).
    pub fn reset(&mut self) {
        self.points.fill(Point::default());
        self.d1 = 0.0;
        self.d2 = 0.0;
        self.have_d1 = false;
        self.have_d2 = false;
        self.stats = 0;
        self.gain = 1.0;
        self.soft_noise = None;
        self.newest = None;
    }

    /// The averaging gain in use.
    pub fn gain(&self) -> f64 {
        self.gain
    }

    /// The noise of one pilot `r` and the change of the channel between refreshes `q`, once statistics exist.
    pub fn noise_and_change(&self) -> Option<(f64, f64)> {
        self.have_d2.then(|| {
            let r = ((4.0 * self.d1 - self.d2) / 6.0).max(1e-12);
            let q = ((self.d2 - self.d1) / 3.0).max(0.0);
            (r, q)
        })
    }

    /// The smoothed noise for the soft bits, `r x pilot_boost` (before the caller's calibration).
    pub fn soft_noise(&self) -> Option<f64> {
        self.soft_noise
    }

    /// The statistics taken so far.
    pub fn statistics(&self) -> u32 {
        self.stats
    }

    /// The newest symbol given.
    pub fn newest(&self) -> Option<i64> {
        self.newest
    }

    /// Gives one statistic: `m1` the mean square change one lag apart, `m2` two lags apart (with no `m2`, only `d1` moves). Used
    /// by [`push_symbol`](Self::push_symbol) with the scattered pilots, or by the caller when it measures them elsewhere.
    pub fn add_statistics(&mut self, m1: f64, m2: Option<f64>) {
        let s = self.cfg.smoothing;
        if self.have_d1 {
            self.d1 += s * (m1 - self.d1);
        } else {
            self.d1 = m1;
            self.have_d1 = true;
        }
        if let Some(m2) = m2 {
            if self.have_d2 {
                self.d2 += s * (m2 - self.d2);
            } else {
                self.d2 = m2;
                self.have_d2 = true;
            }
            self.stats += 1;
        }
        if let Some((r, _)) = self.noise_and_change() {
            let n = r * self.cfg.pilot_boost;
            self.soft_noise = Some(match self.soft_noise {
                Some(v) => 0.9 * v + 0.1 * n,
                None => n,
            });
        }
        self.choose_gain();
    }

    /// The gain search of step 3.
    fn choose_gain(&mut self) {
        let Some((r, q)) = self
            .noise_and_change()
            .filter(|_| self.stats >= self.cfg.min_stats)
        else {
            self.gain = 1.0;
            return;
        };
        let w = self.cfg.change_weight;
        let mut best = (1.0f64, f64::MAX);
        for i in 1..=20 {
            let a = i as f64 * 0.05;
            let cost = w * q * ((1.0 - a) / a).powi(2) + r * a / (2.0 - a);
            if cost < best.1 {
                best = (a, cost);
            }
        }
        self.gain = best.0.max(self.cfg.min_gain);
    }

    /// Takes the raw pilots `(grid point, Y / pilot value)` of symbol `symbol` (increasing from call to call). Errors for a
    /// point beyond the grid or a symbol that goes back.
    pub fn push_symbol(
        &mut self,
        symbol: i64,
        pilots: &[(usize, Complex32)],
    ) -> Result<(), ModemError> {
        if self.newest.is_some_and(|t| symbol < t) {
            return Param::Symbols.not_allowed(symbol as f64);
        }
        if let Some(&(k, _)) = pilots.iter().find(|p| p.0 >= self.points.len()) {
            return Param::Points.range(0.0, (self.points.len() - 1) as f64, k as f64);
        }
        self.newest = Some(symbol);
        let lag = self.cfg.refresh;
        if self.cfg.scattered_statistics {
            let (mut s1, mut n1, mut s2, mut n2) = (0f64, 0usize, 0f64, 0usize);
            for &(k, p) in pilots {
                let pt = &self.points[k];
                if let Some((_, v)) = pt.raw1.filter(|r| r.0 == symbol - lag) {
                    s1 += (p - v).norm_sqr() as f64;
                    n1 += 1;
                }
                if let Some((_, v)) = pt.raw2.filter(|r| r.0 == symbol - 2 * lag) {
                    s2 += (p - v).norm_sqr() as f64;
                    n2 += 1;
                }
            }
            if n1 > 0 {
                self.add_statistics(s1 / n1 as f64, (n2 > 0).then(|| s2 / n2 as f64));
            }
        }
        let a = self.gain as f32;
        let cap = self.cfg.count_cap;
        let restart = self.cfg.restart_after;
        for &(k, p) in pilots {
            let pt = &mut self.points[k];
            if pt.raw1.is_some_and(|r| r.0 == symbol) {
                continue;
            }
            pt.raw2 = pt.raw1;
            pt.raw1 = Some((symbol, p));
            if pt.count == 0 || symbol - pt.last > restart {
                pt.value = p;
                pt.count = 1;
                pt.len = 0;
            } else {
                pt.count = (pt.count + 1).min(cap);
                let g = a.max(1.0 / pt.count as f32);
                pt.value += (p - pt.value) * g;
            }
            pt.last = symbol;
            let v = pt.value;
            pt.store(symbol, v);
        }
        Ok(())
    }

    /// The channel of every grid point at symbol `symbol` into `out` (the grid size); `false` while some point has no value yet
    /// or `out` has another size.
    pub fn channel_at(&self, symbol: i64, out: &mut [Complex32]) -> bool {
        if out.len() != self.points.len() {
            return false;
        }
        for (o, p) in out.iter_mut().zip(&self.points) {
            match p.at(symbol, self.cfg.interpolate) {
                Some(v) => *o = v,
                None => return false,
            }
        }
        true
    }
}

/// The channel of a pilot present in every symbol (the carrier above the band): the mean of its values at `m - 1`, `m`, `m + 1`.
pub fn continual_mean(before: Complex32, at: Complex32, after: Complex32) -> Complex32 {
    (before + at + after) / 3.0
}

/// The rule of [`delay_window`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DelayWindowRule {
    /// The floor is this multiple of the median tap power in the search range (1.5).
    pub floor_factor: f64,
    /// The impulse response is valid when its peak is at least this multiple of the floor (30).
    pub valid: f64,
    /// Paths are taps above this multiple of the floor (12) ...
    pub path: f64,
    /// ... and above this share of the peak (3e-4).
    pub path_of_peak: f64,
    /// The half width is `spread x span + margin` (0.7) ...
    pub spread: f64,
    /// ... (48 samples).
    pub margin: f64,
    /// The window is used when its half width is under this share of `N / 6` (0.9).
    pub limit: f64,
    /// The search range is `+-(N / 6 - edge)` samples (16).
    pub edge: f64,
}

impl Default for DelayWindowRule {
    fn default() -> Self {
        DelayWindowRule {
            floor_factor: 1.5,
            valid: 30.0,
            path: 12.0,
            path_of_peak: 3e-4,
            spread: 0.7,
            margin: 48.0,
            limit: 0.9,
            edge: 16.0,
        }
    }
}

/// The delay window from an impulse response `ir` of `N` taps (the inverse FFT of the full-window channel estimate, tap `d` at
/// `ir[d mod N]`): over the delays within `+-(N / 6 - edge)`, the floor is `floor_factor` times the median tap power; when the
/// peak is at least `valid` times the floor, the paths are the taps above `max(path x floor, path_of_peak x peak)`, the centre is
/// the middle of the first and last path and the half width `spread x span + margin`. Returns `(centre, half width)` in samples
/// when the half width is under `limit x N / 6`, `None` otherwise (keep the full window). `scratch` grows once.
pub fn delay_window(
    ir: &[Complex32],
    rule: &DelayWindowRule,
    scratch: &mut Vec<f64>,
) -> Option<(f64, f64)> {
    let n = ir.len();
    let reach = (n as f64 / 6.0 - rule.edge).floor() as i64;
    if n == 0 || reach < 1 {
        return None;
    }
    let power = |d: i64| ir[d.rem_euclid(n as i64) as usize].norm_sqr() as f64;
    scratch.clear();
    scratch.extend((-reach..=reach).map(power));
    let mid = scratch.len() / 2;
    let median = *scratch.select_nth_unstable_by(mid, f64::total_cmp).1;
    let floor = rule.floor_factor * median;
    let peak = (-reach..=reach).map(power).fold(0.0, f64::max);
    if !(peak > 0.0 && peak >= rule.valid * floor) {
        return None;
    }
    let thr = (rule.path * floor).max(rule.path_of_peak * peak);
    let first = (-reach..=reach).find(|&d| power(d) > thr)?;
    let last = (-reach..=reach).rev().find(|&d| power(d) > thr)?;
    let centre = 0.5 * (first + last) as f64;
    let half = rule.spread * (last - first) as f64 + rule.margin;
    (half < rule.limit * n as f64 / 6.0).then_some((centre, half))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_algo_dsp::fft::Fft;
    use sdrkit_algo_sim::rng::Rng;
    use std::f64::consts::TAU;

    /// A channel of two paths per grid point at symbol `t`: Doppler `fd` (cycles per symbol) on the second path, a delay that turns
    /// the second path by 0.7 rad from point to point.
    fn channel(k: usize, t: i64, fd: f64) -> Complex32 {
        let a = Complex32::from_polar(0.8, (TAU * 0.3 * fd * t as f64) as f32);
        let b = Complex32::from_polar(0.6, (0.7 * k as f64 + TAU * fd * t as f64) as f32);
        a + b
    }

    /// Mean square error of the shared estimator and of a newest-pilot average of weight 0.5 (a point refreshed every 4
    /// symbols, so 0 to 3 symbols old), over `n` symbols at pilot noise `var` and Doppler `fd`.
    fn errors(fd: f64, var: f64, n: i64, seed: u64) -> (f64, f64, f64) {
        let pts = 120;
        let mut est = PilotTimeInterp::new(PilotTimeConfig::new(pts)).unwrap();
        let mut simple = vec![None::<Complex32>; pts];
        let mut rng = Rng::new(seed);
        let sd = (var / 2.0).sqrt();
        let mut pil = Vec::new();
        let mut out = vec![Complex32::default(); pts];
        let (mut e_new, mut e_old, mut count) = (0f64, 0f64, 0usize);
        for t in 0..n {
            pil.clear();
            for k in (0..pts).filter(|k| (*k as i64 - t).rem_euclid(4) == 0) {
                let noise = Complex32::new((sd * rng.normal()) as f32, (sd * rng.normal()) as f32);
                let p = channel(k, t, fd) + noise;
                pil.push((k, p));
                let s = &mut simple[k];
                *s = Some(s.map_or(p, |v| v + (p - v) * 0.5));
            }
            est.push_symbol(t, &pil).unwrap();
            let m = t - 8;
            if t < 200 || !est.channel_at(m, &mut out) {
                continue;
            }
            for k in 0..pts {
                e_new += (out[k] - channel(k, m, fd)).norm_sqr() as f64;
                // The simple form serves symbol t at once.
                e_old += (simple[k].unwrap() - channel(k, t, fd)).norm_sqr() as f64;
                count += 1;
            }
        }
        (e_new / count as f64, e_old / count as f64, est.gain())
    }

    #[test]
    fn time_interpolation_beats_the_newest_pilot_average() {
        // Pilot SNR 15 dB. Doppler of 10 Hz and 70 Hz at the 1.26 ms symbols of ISDB-T with the 8K FFT and guard 1/4.
        let var = 10f64.powf(-1.5);
        let static_ = errors(0.0, var, 2000, 1);
        let slow = errors(10.0 * 1.26e-3, var, 2000, 2);
        let fast = errors(70.0 * 1.26e-3, var, 2000, 3);
        // The shared estimate has under half the error of the newest-pilot average in each case, and a static channel gets a
        // smaller gain than a moving one.
        assert!(
            static_.0 < 0.5 * static_.1 && static_.2 < slow.2,
            "{static_:?}"
        );
        assert!(slow.0 < 0.35 * slow.1, "{slow:?}");
        assert!(fast.0 < 0.5 * fast.1, "{fast:?}");
    }

    #[test]
    fn statistics_give_the_noise_and_the_gain_search_follows() {
        // No change, pilot noise 0.02: r reads 0.02 and q about 0; the gain falls well below 1.
        let pts = 64;
        let mut est = PilotTimeInterp::new(PilotTimeConfig::new(pts)).unwrap();
        let mut rng = Rng::new(9);
        let sd = 0.1f64;
        for t in 0..400 {
            let pil: Vec<(usize, Complex32)> = (0..pts)
                .filter(|k| (*k as i64 - t).rem_euclid(4) == 0)
                .map(|k| {
                    (
                        k,
                        Complex32::new(
                            1.0 + (sd * rng.normal()) as f32,
                            (sd * rng.normal()) as f32,
                        ),
                    )
                })
                .collect();
            est.push_symbol(t, &pil).unwrap();
        }
        let (r, q) = est.noise_and_change().unwrap();
        assert!((r / 0.02 - 1.0).abs() < 0.25 && q < 0.003, "{r} {q}");
        assert!(est.gain() < 0.5, "{}", est.gain());
        assert!((est.soft_noise().unwrap() / (0.02 * 16.0 / 9.0) - 1.0).abs() < 0.25);
        assert!(est.push_symbol(10, &[]).is_err());
        assert!(est.push_symbol(400, &[(64, Complex32::default())]).is_err());
        // Time interpolation off: the newest average.
        let mut cfg = PilotTimeConfig::new(1);
        cfg.interpolate = false;
        let mut e = PilotTimeInterp::new(cfg).unwrap();
        e.push_symbol(0, &[(0, Complex32::new(1.0, 0.0))]).unwrap();
        e.push_symbol(4, &[(0, Complex32::new(3.0, 0.0))]).unwrap();
        let mut o = [Complex32::default()];
        assert!(e.channel_at(0, &mut o));
        // Before any statistics the gain is 1: the newest pilot.
        assert_eq!(o[0], Complex32::new(3.0, 0.0));
        let c = |v: f32| Complex32::new(v, 0.0);
        assert_eq!(continual_mean(c(1.0), c(3.0), c(5.0)), c(3.0));
    }

    #[test]
    fn delay_window_spans_the_paths() {
        // 2K FFT, paths at 10 and 130 samples, a noise floor.
        let n = 2048;
        let mut h = vec![Complex32::default(); n];
        let mut rng = Rng::new(4);
        for v in h.iter_mut() {
            *v = Complex32::new((1e-3 * rng.normal()) as f32, (1e-3 * rng.normal()) as f32);
        }
        h[10] += Complex32::new(1.0, 0.0);
        h[130] += Complex32::new(0.0, 0.3);
        let mut s = Vec::new();
        let (c, half) = delay_window(&h, &DelayWindowRule::default(), &mut s).unwrap();
        assert_eq!(c, 70.0);
        assert!((half - (0.7 * 120.0 + 48.0)).abs() < 1e-9);
        // An echo too far apart for a narrow window keeps the full one; pure noise is not valid.
        h[n - 300] += Complex32::new(0.5, 0.0);
        assert!(delay_window(&h, &DelayWindowRule::default(), &mut s).is_none());
        let noise: Vec<Complex32> = (0..n)
            .map(|_| Complex32::new(rng.normal() as f32, 0.0))
            .collect();
        assert!(delay_window(&noise, &DelayWindowRule::default(), &mut s).is_none());
        // The impulse response of an interpolated channel: one path at 25 samples.
        let mut fft = Fft::inverse(n);
        let mut spec: Vec<Complex32> = (0..n)
            .map(|k| Complex32::from_polar(1.0, (-TAU * 25.0 * k as f64 / n as f64) as f32))
            .collect();
        fft.process(&mut spec);
        let (c, _) = delay_window(&spec, &DelayWindowRule::default(), &mut s).unwrap();
        assert_eq!(c, 25.0);
    }
}
