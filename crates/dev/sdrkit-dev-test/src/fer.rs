//! Frame error rate of a set of decoders of one code against the signal to noise ratio: points on a fixed grid or found by
//! search, the ratio at which the error rate falls to a target, and the printed tables. The decoders of a point all decode
//! the same frames, so their rates compare without the noise of separate draws.

use sdrkit_algo_sim::noise::Awgn;
use sdrkit_core::Complex32;

/// The min-sum scales of an LDPC decoder that the measurements compare, in sixteenths.
pub const SCALES: [u8; 5] = [12, 13, 14, 15, 16];

/// The frames a point: the value of `LDPC_SCALE_FRAMES` when it is set and a number, else `default`.
pub fn frames(default: u64) -> u64 {
    std::env::var("LDPC_SCALE_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// One code under test with a set of decoders.
pub trait Trial {
    /// Starts a point at `db` (the caller's measure: Es/N0 or Eb/N0); the frames of the point draw from `seed`.
    fn start(&mut self, db: f64, seed: u64);
    /// Makes frame `f` of the point and sends it through the channel.
    fn send(&mut self, f: u64);
    /// Decodes the last frame with decoder `i`; true when the receiver loses it.
    fn lost(&mut self, i: usize) -> bool;
}

/// The lost frames of each decoder at one point.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// The signal to noise ratio, dB.
    pub db: f64,
    /// The frames sent.
    pub frames: u64,
    /// The lost frames, per decoder.
    pub lost: Vec<u64>,
}

impl Point {
    /// The frame error rate of decoder `i`.
    pub fn fer(&self, i: usize) -> f64 {
        self.lost[i] as f64 / self.frames as f64
    }
}

/// Adds complex white noise of power `10^(-snr_db / 10)` to `cells` (unit mean power) and sets every weight to `1 / N0`.
pub fn noise(cells: &mut [Complex32], weights: &mut [f32], snr_db: f32, seed: u64) {
    let n0 = 10f32.powf(-snr_db / 10.0);
    Awgn::with_power(n0 as f64, seed)
        .unwrap_or_else(|_| unreachable!("the noise power is positive"))
        .process(cells);
    weights.fill(1.0 / n0);
}

/// Runs `f` on every item of `items`, one thread per item, and returns the results in order.
pub fn parallel<J: Sync, R: Send>(items: &[J], f: impl Fn(&J) -> R + Sync) -> Vec<R> {
    std::thread::scope(|s| {
        let jobs: Vec<_> = items.iter().map(|j| s.spawn(|| f(j))).collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or_else(|_| panic!("a measurement thread failed")))
            .collect()
    })
}

/// Sends `frames` frames at `db` from `seed` and counts the lost frames of `decoders` decoders. With `probe`, the point
/// stops after `probe` frames when every decoder lost all of them, and counts as all lost.
pub fn point<T: Trial>(t: &mut T, decoders: usize, db: f64, seed: u64, frames: u64, probe: Option<u64>) -> Point {
    t.start(db, seed);
    let mut lost = vec![0u64; decoders];
    for f in 0..frames {
        if probe == Some(f) && lost.iter().all(|&n| n == f) {
            return Point { db, frames: f, lost };
        }
        t.send(f);
        for (i, l) in lost.iter_mut().enumerate() {
            *l += u64::from(t.lost(i));
        }
    }
    Point { db, frames, lost }
}

/// The points of `dbs` with `frames` frames each, one thread and one trial of `make` per point; point `p` draws from seed
/// `p + 1`.
pub fn grid<T: Trial>(make: impl Fn() -> T + Sync, decoders: usize, dbs: &[f64], frames: u64) -> Vec<Point> {
    let at: Vec<(usize, f64)> = dbs.iter().copied().enumerate().collect();
    parallel(&at, |&(p, db)| point(&mut make(), decoders, db, p as u64 + 1, frames, None))
}

/// The points around the waterfall, found from `from_db`: 0.25 dB steps of 100 frames up to the first where the best
/// decoder loses at most half (or up to `to_db`), then 0.1 dB steps of `frames` frames from 0.5 dB below it until every
/// decoder is under a quarter of `target` (at most 40 steps). A fine point stops after 100 frames that every decoder lost.
pub fn search<T: Trial>(t: &mut T, decoders: usize, from_db: f64, to_db: f64, frames: u64, target: f64) -> Vec<Point> {
    const PROBE: u64 = 100;
    let mut mid = from_db;
    while mid < to_db {
        let p = point(t, decoders, mid, 1, PROBE, Some(PROBE));
        if (0..decoders).map(|i| p.fer(i)).fold(1.0, f64::min) <= 0.5 {
            break;
        }
        mid += 0.25;
    }
    let mut points = Vec::new();
    for i in 0..40u64 {
        let p = point(t, decoders, mid - 0.5 + 0.1 * i as f64, 0x5CA1E ^ (i + 1), frames, Some(PROBE));
        let done = (0..decoders).all(|d| p.fer(d) < target / 4.0);
        points.push(p);
        if done {
            break;
        }
    }
    points
}

/// The ratio at which the frame error rate of decoder `i` falls to `target`: the first point at or below it after a point
/// above it, interpolated on log FER (a rate of 0 counts as half a frame); `None` when it does not cross in `points`.
pub fn threshold(points: &[Point], i: usize, target: f64) -> Option<f64> {
    let fer = |p: &Point| p.fer(i).max(0.5 / p.frames as f64).log10();
    let w = points.windows(2).find(|w| fer(&w[0]) > target.log10() && fer(&w[1]) <= target.log10())?;
    let (a, b) = (fer(&w[0]), fer(&w[1]));
    Some(w[0].db + (a - target.log10()) / (a - b) * (w[1].db - w[0].db))
}

/// The table of a grid: the frame error rate per point and scale (without the points where every scale loses all frames or
/// none), then the ratio at each of `targets` per scale.
pub fn table(name: &str, scales: &[u8], points: &[Point], targets: &[f64]) -> String {
    let frames = points.first().map_or(0, |p| p.frames);
    let mut t = format!("{name}, {frames} frames a point; dB, then FER at scale/16 =");
    for s in scales {
        t += &format!(" {s:6}");
    }
    t.push('\n');
    for p in points {
        if p.lost.iter().all(|&n| n == 0) || p.lost.iter().all(|&n| n == p.frames) {
            continue;
        }
        t += &format!("  {:6.2}:", p.db);
        for i in 0..scales.len() {
            t += &format!(" {:6.3}", p.fer(i));
        }
        t.push('\n');
    }
    for &target in targets {
        t += &format!("  FER {target:4}:");
        for i in 0..scales.len() {
            t += &threshold(points, i, target).map_or("      -".into(), |x| format!(" {x:6.2}"));
        }
        t.push('\n');
    }
    t
}

/// One line of a search: the ratio at `target` per scale, then the scale with the lowest ratio and its gain over 13/16.
pub fn line(name: &str, scales: &[u8], points: &[Point], target: f64) -> String {
    let thr: Vec<Option<f64>> = (0..scales.len()).map(|i| threshold(points, i, target)).collect();
    let mut line = format!("{name}:");
    for t in &thr {
        line += &t.map_or("   --  ".into(), |v| format!(" {v:6.2}"));
    }
    let best = (0..scales.len()).filter_map(|i| Some((i, thr[i]?))).min_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((b, v)) = best {
        line += &format!("  best {}", scales[b]);
        if let Some(base) = scales.iter().position(|&s| s == 13).and_then(|i| thr[i]) {
            line += &format!(" gain {:.2} dB", base - v);
        }
    }
    line
}
