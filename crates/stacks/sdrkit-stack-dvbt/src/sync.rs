//! `dvbt.sync`: symbol timing from the cyclic prefix, the fractional carrier offset loop, resynchronisation and window placement
//! under echoes. One N-sample window for each symbol leaves as a [`SymbolBlock`] item.
//!
//! Standard: ETSI EN 300 744 clause 4.4.

use crate::items::{Link, Restart, SymbolBlock, Timing};
use crate::typed::SyncFields;
use sdrkit_core::interface::Fields;
use sdrkit_core::publish::{field, Published};
use sdrkit_core::{
    Complex32, ConfigError, Results, Setting, SettingError, SettingId, SettingSet, Value, Values,
};
use sdrkit_graph::{
    values_of, Applied, Component, ComponentInfo, Format, Io, Kind, Port, RateFrom, TagKind, Timed,
};
use sdrkit_graph::{Kept, SlicePool};
use std::f64::consts::PI;
use std::sync::Arc;

/// The echo profile and the state of the window placement under echoes.
#[derive(Clone, Debug, Default)]
struct Profile {
    acc: Vec<f64>,
    /// The buffer that `acc` is swapped with when it is examined.
    spare: Vec<f64>,
    symbols: u32,
    examined_once: bool,
    tainted: bool,
    plateau: bool,
    moved_once: bool,
    /// Accumulated window moves since the profile started (to keep it on a grid fixed in input samples).
    offset: i64,
}

impl Profile {
    /// A profile of `len` offsets with both of its buffers, so that no later step allocates.
    fn sized(len: usize) -> Self {
        Profile {
            acc: vec![0.0; len],
            spare: Vec::with_capacity(len),
            ..Profile::default()
        }
    }

    /// Starts the profile again in the buffers that it has.
    fn restart(&mut self) {
        let len = self.acc.len();
        let acc = std::mem::take(&mut self.acc);
        let spare = std::mem::take(&mut self.spare);
        *self = Profile {
            acc,
            spare,
            ..Profile::default()
        };
        self.acc.clear();
        self.acc.resize(len, 0.0);
    }
}

/// The offsets of the echo profile for a guard of `g` samples: 2G + G/8 to each side.
fn profile_len(g: usize) -> usize {
    2 * (2 * g + g / 8) + 1
}

/// A least-squares line through the symbol boundaries that the cyclic prefix measured, against the symbol count: its
/// slope is the drift of the sample clock in samples for each symbol. The moves of the window by the timing loop are
/// in the boundaries, so the slope does not depend on the loop.
#[derive(Clone, Copy, Debug, Default)]
struct DriftFit {
    /// Where the first symbol started, and the symbols since.
    origin: Option<u64>,
    k: u64,
    /// Boundaries in the sums.
    n: u64,
    sk: f64,
    sx: f64,
    skk: f64,
    skx: f64,
}

impl DriftFit {
    /// The least boundaries for a slope, in symbols of 8K in time: the noise of the slope is then under 0.5 ppm.
    const LEAST: u64 = 120;
    /// The fit starts again after this many, so that it follows a clock that changes.
    const MOST: u64 = 1024;

    /// Adds the boundary of the symbol that the loop put at `s` and the cyclic prefix found `tau` samples later.
    fn add(&mut self, s: u64, tau: f64, period: usize) {
        let origin = *self.origin.get_or_insert(s);
        let k = self.k as f64;
        let x = s as f64 - origin as f64 - k * period as f64 + tau;
        self.n += 1;
        self.sk += k;
        self.sx += x;
        self.skk += k * k;
        self.skx += k * x;
    }

    /// Counts a symbol, with a boundary added or not.
    fn step(&mut self) {
        if self.origin.is_some() {
            self.k += 1;
        }
    }

    /// The slope, samples for each symbol.
    fn slope(&self) -> Option<f64> {
        let n = self.n as f64;
        let den = n * self.skk - self.sk * self.sk;
        (den > 0.0).then(|| (n * self.skx - self.sk * self.sx) / den)
    }
}

#[derive(Clone, Debug)]
struct Track {
    n: usize,
    g: usize,
    p: usize,
    b: usize,
    /// Symbols in the time of one 8K symbol: 1 for 8K, 4 for 2K.
    per_8k: u64,
    /// The line through the measured symbol boundaries, for the drift of the sample clock at sizes under 8K, and the
    /// line that takes its place when it is full. The second starts when the first is half full, so the line in use
    /// always has half of the boundaries or more behind it.
    fit: DriftFit,
    fit_next: DriftFit,
    s: u64,
    accum: f64,
    integ: f64,
    frac: f64,
    phase: f64,
    phase_at: u64,
    count: u64,
    since: u64,
    rho_avg: f64,
    rho_n: u64,
    collapsed_run: u32,
    edge_run: u32,
    last_window: Option<u64>,
    fresh: bool,
    resynced: bool,
    rebuild: bool,
    int_step: i32,
    profile: Profile,
    peak_only: bool,
    spread: usize,
}

impl Track {
    /// Starts the line fit again: the window jumped, or the resampler got a correction.
    fn restart_fit(&mut self) {
        self.fit = DriftFit::default();
        self.fit_next = DriftFit::default();
    }
}

/// `dvbt.sync`.
pub struct DvbtSync {
    link: Arc<Link>,
    rate: f64,
    buf: Vec<Complex32>,
    base: u64,
    end: u64,
    st: Option<Track>,
    pool: SlicePool<Complex32>,
    table: Vec<Complex32>,
    corr: Vec<(Complex32, f64)>,
    p: Published,
    /// Resynchronisations so far.
    pub resyncs: u64,
    /// Last timing error (samples), correlation coefficient, fractional offset.
    pub last: (f64, f64, f64),
    /// The drift of the sample clock that the resampler does not correct, ppm: what the timing loop holds in 8K, the
    /// slope of the line fit at the smaller sizes.
    pub drift_ppm: f64,
    /// Window placement from the correlation peak only (a test setting: no placement under echoes).
    pub peak_only: bool,
}

/// The window from the correlation peak only.
pub const PEAK_ONLY: Setting<field::Bool> = Setting::new(
    "peak_only",
    "Window from the correlation peak only",
    "",
    false,
);

sdrkit_core::settings! {
    /// The settings of `dvbt.sync`.
    pub enum SyncSetting {
        /// The window from the correlation peak only.
        PeakOnly = PEAK_ONLY,
    }
}

impl Default for DvbtSync {
    fn default() -> Self {
        DvbtSync::new(Link::new())
    }
}

/// Samples needed beyond the start of a symbol before it is processed: the echo profile reaches 2G + G/8 past it, and the
/// resynchronisation search a full period around the next one.
fn lookahead(t: &Track) -> u64 {
    (2 * t.p + t.p / 2 + 3 * t.g + 64) as u64
}

impl DvbtSync {
    /// A synchroniser sharing `link`.
    pub fn new(link: Arc<Link>) -> Self {
        DvbtSync {
            link,
            rate: 1.0,
            buf: Vec::new(),
            base: 0,
            end: 0,
            st: None,
            pool: SlicePool::default(),
            table: Vec::new(),
            corr: Vec::new(),
            p: Published::new(SyncFields::SHAPE.fields),
            resyncs: 0,
            last: (0.0, 0.0, 0.0),
            drift_ppm: 0.0,
            peak_only: false,
        }
    }

    /// Drops the samples before `at` (a restart: they belong to another mixer offset or frequency).
    fn forget_before(&mut self, at: u64) {
        if at > self.base {
            let d = ((at - self.base) as usize).min(self.buf.len());
            self.buf.drain(..d);
            self.base += d as u64;
        }
    }

    /// The fractional carrier offset in use, subcarrier spacings (0 when not tracking).
    pub fn frac(&self) -> f64 {
        self.st.as_ref().map_or(0.0, |t| t.frac)
    }

    fn start(&mut self, t: Timing) {
        let g = t.guard;
        // Room for the search of a whole period and for the echo profile: no symbol after this allocates for them.
        self.corr.clear();
        self.corr.reserve(t.fft + g + 1);
        self.st = Some(Track {
            n: t.fft,
            g,
            p: t.fft + g,
            b: (g / 4).min(24),
            per_8k: (8192 / t.fft.max(1)).max(1) as u64,
            fit: DriftFit::default(),
            fit_next: DriftFit::default(),
            s: t.start.max(self.base + 8),
            accum: 0.0,
            integ: 0.0,
            frac: t.frac,
            phase: 0.0,
            phase_at: t.start,
            count: 0,
            since: 0,
            rho_avg: 0.0,
            rho_n: 0,
            collapsed_run: 0,
            edge_run: 0,
            last_window: None,
            fresh: true,
            resynced: false,
            rebuild: false,
            int_step: 0,
            profile: Profile::sized(profile_len(g)),
            peak_only: false,
            spread: 0,
        });
    }

    #[inline]
    fn at(&self, i: u64) -> Complex32 {
        self.buf[(i - self.base) as usize]
    }

    /// Cyclic-prefix correlation over G samples starting at `s`: (sum of y[i] conj(y[i + N]), energy of both copies / 2).
    fn cp(&self, s: u64, n: usize, g: usize) -> (Complex32, f64) {
        let i0 = (s - self.base) as usize;
        let a = &self.buf[i0..i0 + g];
        let b = &self.buf[i0 + n..i0 + n + g];
        let mut c = Complex32::default();
        let mut e = 0f32;
        for (x, y) in a.iter().zip(b) {
            c += x * y.conj();
            e += x.norm_sqr() + y.norm_sqr();
        }
        (c, 0.5 * e as f64)
    }

    /// The correlation at every offset from `s - lo` to `s + hi` (inclusive), by one full sum and a sliding update.
    fn sweep(&mut self, s: u64, lo: usize, hi: usize, n: usize, g: usize) {
        self.corr.clear();
        let start = s - lo as u64;
        let (mut c, mut e) = self.cp(start, n, g);
        let (mut cd, mut ed) = (Complex32::default(), 0f64);
        let _ = (&mut cd, &mut ed);
        self.corr.push((c, e));
        for k in 1..=(lo + hi) {
            let i = start + k as u64 - 1;
            let out_a = self.at(i);
            let out_b = self.at(i + n as u64);
            let in_a = self.at(i + g as u64);
            let in_b = self.at(i + g as u64 + n as u64);
            c += in_a * in_b.conj() - out_a * out_b.conj();
            e += 0.5
                * (in_a.norm_sqr() + in_b.norm_sqr() - out_a.norm_sqr() - out_b.norm_sqr()) as f64;
            // Re-sum every 256 steps so rounding never accumulates.
            if k % 256 == 0 {
                let (c2, e2) = self.cp(start + k as u64, n, g);
                c = c2;
                e = e2;
            }
            self.corr.push((c, e));
        }
    }

    /// Searches one full symbol period around `s` for the boundary: (shift nearest zero, correlation coefficient).
    fn full_search(&mut self, t: &Track) -> Option<(i64, f64)> {
        let half = t.p / 2;
        if t.s < self.base + half as u64 || t.s + (half + t.g + t.n + 8) as u64 > self.end {
            return None;
        }
        self.sweep(t.s, half, t.p - half - 1, t.n, t.g);
        let mut best = (0usize, 0f64);
        for (k, &(c, e)) in self.corr.iter().enumerate() {
            let r = if e > 0.0 { c.norm() as f64 / e } else { 0.0 };
            if r > best.1 {
                best = (k, r);
            }
        }
        Some((best.0 as i64 - half as i64, best.1))
    }

    fn emit(
        &mut self,
        t: &mut Track,
        w: u64,
        collapsed: bool,
        rho: f64,
        time_rate: f64,
        out: &mut Vec<Timed<SymbolBlock>>,
    ) {
        let n = t.n;
        // Exact derotation: the phase in double precision at every 64-sample block, a table inside it.
        let step = -2.0 * PI * t.frac / n as f64;
        if self.table.len() != 64 {
            self.table = vec![Complex32::default(); 64];
        }
        for (i, v) in self.table.iter_mut().enumerate() {
            let a = step * i as f64;
            *v = Complex32::new(a.cos() as f32, a.sin() as f32);
        }
        let phase_w = t.phase + step * (w as f64 - t.phase_at as f64);
        let i0 = (w - self.base) as usize;
        let src = &self.buf[i0..i0 + n];
        let table = &self.table;
        let samples = self.pool.exact(n, |v| {
            for (blk, (chunk, o)) in src.chunks(64).zip(v.chunks_mut(64)).enumerate() {
                let a = phase_w + step * (blk * 64) as f64;
                let r = Complex32::new(a.cos() as f32, a.sin() as f32);
                for (k, (&x, o)) in chunk.iter().zip(o.iter_mut()).enumerate() {
                    *o = x * (r * table[k]);
                }
            }
        });
        let moved = match t.last_window {
            Some(l) => w as i64 - (l + t.p as u64) as i64,
            None => 0,
        };
        t.last_window = Some(w);
        out.push(Timed {
            time: w as f64 / time_rate,
            value: SymbolBlock {
                samples,
                fft: n,
                guard: t.g,
                count: t.count,
                window: w,
                moved,
                collapsed,
                resynced: std::mem::take(&mut t.resynced),
                fresh: std::mem::take(&mut t.fresh),
                int_step: std::mem::take(&mut t.int_step),
                frac: t.frac,
                rho: rho as f32,
                time: w as f64 / time_rate,
                rebuild: std::mem::take(&mut t.rebuild),
                rate: time_rate,
            },
        });
        t.count += 1;
    }

    fn symbol(&mut self, t: &mut Track, out: &mut Vec<Timed<SymbolBlock>>) -> bool {
        let s = t.s;
        let (n, g, p) = (t.n, t.g, t.p);
        self.sweep(s, 6, 6, n, g);
        let mut mags = [0f64; 13];
        for (m, c) in mags.iter_mut().zip(&self.corr) {
            *m = c.0.norm() as f64;
        }
        let (k, m) = mags
            .iter()
            .copied()
            .enumerate()
            .fold(
                (6usize, -1.0f64),
                |b, (i, v)| if v > b.1 { (i, v) } else { b },
            );
        let mut delta = 0.0;
        if k > 0 && k < 12 {
            let (a, c) = (mags[k - 1], mags[k + 1]);
            let den = a - 2.0 * m + c;
            if den < 0.0 {
                delta = (0.5 * (a - c) / den).clamp(-0.5, 0.5);
            }
        }
        let tau = k as f64 - 6.0 + delta;
        let (ck, ek) = self.corr[k];
        let rho = if ek > 0.0 { ck.norm() as f64 / ek } else { 0.0 };
        let raw = -(ck.im as f64).atan2(ck.re as f64) / (2.0 * PI);
        let collapsed = t.since >= 40 && rho < 0.3 * t.rho_avg;
        let centre = mags[6];
        let edge = (k == 0 || k == 12) && (!t.profile.plateau || m > 1.05 * centre);
        self.last = (tau, rho, t.frac);
        // Output the window of this symbol.
        let w = s + g as u64 - t.b as u64;
        self.emit(t, w, collapsed, rho, self.rate, out);
        t.since += 1;
        // The echo profile, every second symbol while TPS is locked.
        if self.link.locked() && !self.peak_only && t.since.is_multiple_of(2) {
            self.profile_add(t, collapsed);
        }
        let old_frac = t.frac;
        if collapsed {
            t.collapsed_run += 1;
            t.edge_run = 0;
        } else {
            t.collapsed_run = 0;
            t.edge_run = if edge { t.edge_run + 1 } else { 0 };
            // The usual strength of the correlation.
            let wgt = if t.rho_n < 200 { 0.05 } else { 0.005 };
            t.rho_avg = if t.rho_n == 0 {
                rho
            } else {
                t.rho_avg + wgt * (rho - t.rho_avg)
            };
            t.rho_n += 1;
            // Fractional carrier offset.
            let mut d = raw - t.frac;
            d -= d.round();
            t.frac += 0.15 * d;
            if t.frac > 0.5 {
                t.frac -= 1.0;
                t.int_step += 1;
            } else if t.frac < -0.5 {
                t.frac += 1.0;
                t.int_step -= 1;
            }
            // Timing loop.
            let (kp, ki) = if t.since < 500 {
                (0.08, 0.0016)
            } else {
                (0.01, 0.000025)
            };
            let err = if t.peak_only || t.profile.plateau {
                0.0
            } else {
                tau
            };
            t.integ += ki * err;
            t.accum += kp * err + t.integ;
            // The boundary that this symbol measured, for the line fit. A peak at the edge of the search and a peak on
            // an echo plateau measure nothing.
            if t.per_8k > 1 && !edge && !(t.peak_only || t.profile.plateau) {
                t.fit.add(s, tau, p);
                if 2 * t.fit.n >= DriftFit::MOST * t.per_8k {
                    t.fit_next.add(s, tau, p);
                }
            }
        }
        if collapsed {
            // The learnt drift goes on through a collapsed symbol.
            t.accum += t.integ;
        }
        let mut step = 0i64;
        if t.accum >= 1.0 || t.accum <= -1.0 {
            step = t.accum.trunc() as i64;
            t.accum -= step as f64;
        }
        // Sample clock: a drift above 2 ppm goes to the resampler.
        if t.per_8k == 1 {
            // 8K: the drift that the loop holds, from symbol 60, every 16 symbols.
            self.drift_ppm = t.integ / p as f64 * 1e6;
            if t.since >= 60 && t.since.is_multiple_of(16) {
                let ppm = t.integ / p as f64 * 1e6;
                if ppm.abs() > 2.0 {
                    self.link.add_clock(ppm);
                    t.integ = 0.0;
                }
            }
        } else {
            // A shorter symbol: the noise of the drift that the loop holds passes 2 ppm, so the line fit gives it.
            t.fit.step();
            t.fit_next.step();
            if t.fit.n >= DriftFit::LEAST * t.per_8k {
                if let Some(slope) = t.fit.slope() {
                    self.drift_ppm = slope / p as f64 * 1e6;
                    if t.since.is_multiple_of(16 * t.per_8k) && self.drift_ppm.abs() > 2.0 {
                        self.link.add_clock(self.drift_ppm);
                        self.drift_ppm = 0.0;
                        t.integ = 0.0;
                        t.restart_fit();
                    }
                }
            }
            if t.fit.n >= DriftFit::MOST * t.per_8k {
                t.fit = std::mem::take(&mut t.fit_next);
            }
        }
        let s_next = (s as i64 + p as i64 + step) as u64;
        // The phase runs on with the offset the samples of this symbol were derotated with. A new offset applies from the next.
        t.phase += -2.0 * PI * old_frac / n as f64 * (s_next as f64 - t.phase_at as f64);
        t.phase = t.phase.rem_euclid(2.0 * PI);
        t.phase_at = s_next;
        t.s = s_next;
        // Lost timing, early resynchronisation.
        let lost = t.collapsed_run >= 12;
        let early = self.link.locked() && (t.collapsed_run >= 2 || t.edge_run >= 2);
        if lost || early {
            match self.full_search(t) {
                Some((shift, r)) if r >= (0.3f64).max(0.5 * t.rho_avg) => {
                    let follow = (16i64).max((g as i64 / 16).min(t.spread as i64 + 16));
                    if shift.abs() <= 8 && !lost {
                        // A fade or a burst: nothing to do.
                    } else if shift.abs() <= follow && t.collapsed_run == 0 {
                        // The strongest path moved: the window follows.
                        t.s = (t.s as i64 + shift) as u64;
                        t.phase_at = t.s;
                        t.restart_fit();
                    } else {
                        t.s = (t.s as i64 + shift) as u64;
                        t.phase_at = t.s;
                        t.restart_fit();
                        t.resynced = true;
                        t.rebuild = true;
                        t.accum = 0.0;
                        t.profile.restart();
                        self.resyncs += 1;
                        self.p
                            .results_mut()
                            .log(format!("resynchronised: boundary moved {shift} samples"));
                    }
                    t.collapsed_run = 0;
                    t.edge_run = 0;
                }
                Some(_) | None if lost => {
                    // Not found: mode detection again.
                    self.link.request_restart();
                    self.p
                        .results_mut()
                        .log("timing lost: mode detection again".to_string());
                    return false;
                }
                _ => {
                    t.edge_run = 0;
                }
            }
        }
        true
    }

    fn profile_add(&mut self, t: &mut Track, collapsed: bool) {
        let r = 2 * t.g + t.g / 8;
        let len = profile_len(t.g);
        if collapsed {
            t.profile.tainted = true;
        }
        // The profile around the point the window will use next.
        let s = t.s;
        if s < self.base + r as u64 || s + (r + t.g + t.n + 2) as u64 > self.end {
            return;
        }
        self.sweep(s, r, r, t.n, t.g);
        let off = t.profile.offset;
        for (i, &(c, _)) in self.corr.iter().enumerate() {
            let j = i as i64 + off;
            if j >= 0 && (j as usize) < len {
                t.profile.acc[j as usize] += c.norm() as f64;
            }
        }
        t.profile.symbols += 1;
        let due = if t.profile.examined_once { 12 } else { 6 }; // 12 or 24 symbols, every second one
        if t.profile.symbols >= due {
            self.profile_examine(t);
        }
    }

    fn profile_examine(&mut self, t: &mut Track) {
        let mut pr = std::mem::take(&mut t.profile.spare);
        std::mem::swap(&mut pr, &mut t.profile.acc);
        t.profile.acc.clear();
        t.profile.acc.resize(pr.len(), 0.0);
        self.profile_judge(t, &pr);
        t.profile.spare = pr;
    }

    /// Places the window from the echo profile `pr` that was accumulated.
    fn profile_judge(&mut self, t: &mut Track, pr: &[f64]) {
        let g = t.g as f64;
        let tainted = t.profile.tainted;
        t.profile.symbols = 0;
        t.profile.tainted = false;
        t.profile.examined_once = true;
        let off = t.profile.offset;
        t.profile.offset = 0;
        if tainted || pr.is_empty() {
            return;
        }
        let r = (pr.len() - 1) / 2;
        let nf = (t.g / 8).max(1);
        let floor = pr[..nf].iter().sum::<f64>() / nf as f64;
        let peak = pr.iter().cloned().fold(0.0, f64::max);
        if peak <= floor {
            return;
        }
        let level = (floor * floor + (peak * peak - floor * floor) / 16.0).sqrt();
        let mut rise = None;
        for i in 1..pr.len().saturating_sub(2) {
            if pr[i] >= level && pr[i + 1] >= level && pr[i + 2] >= level {
                let f = if pr[i] > pr[i - 1] {
                    (level - pr[i - 1]) / (pr[i] - pr[i - 1])
                } else {
                    0.0
                };
                rise = Some(i as f64 - 1.0 + f.clamp(0.0, 1.0));
                break;
            }
        }
        let Some(rise) = rise else { return };
        let mut fall = rise;
        for i in (0..pr.len()).rev() {
            if pr[i] >= level {
                fall = i as f64;
                break;
            }
        }
        let top = fall - rise - 0.75 * g;
        t.profile.plateau = top > 0.75 * g + (8.0f64).max(g / 16.0);
        t.spread = (top - 0.75 * g).max(0.0) as usize;
        // The earliest path, relative to the tracked point (profile index r is offset 0 at the start, moved by `off` since).
        let earliest = rise + 0.75 * g - r as f64 - off as f64;
        let mv = earliest.round() as i64;
        let min_move = (8.0f64).max(g / 32.0) as i64;
        if (t.profile.plateau || t.profile.moved_once) && mv.abs() >= min_move {
            t.s = (t.s as i64 + mv) as u64;
            t.phase_at = t.s;
            t.restart_fit();
            t.profile.moved_once = true;
            t.peak_only = t.profile.plateau;
            self.p.results_mut().log(format!(
                "window placed on the earliest path: {mv} samples (plateau {})",
                t.profile.plateau
            ));
        }
        t.peak_only = t.profile.plateau;
    }
}

impl Component for DvbtSync {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "dvbt.sync",
            vec![Port::iq("in")],
            vec![Port::typed::<SymbolBlock>("out").rate_from(RateFrom::Fixed(0.0))],
        )
        .manual_tags()
        .with_settings(SyncSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        self.rate = inputs[0].rate.max(1.0);
        self.buf.clear();
        self.base = 0;
        self.end = 0;
        self.st = None;
        Ok(vec![Format::new(Kind::typed::<SymbolBlock>(), 0.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let input = io.input(0);
        let first = input.first();
        let x = io.iq(0);
        if self.buf.is_empty() {
            self.base = first;
        }
        self.buf.extend_from_slice(x);
        self.end = first + x.len() as u64;
        for t in input.tags() {
            match &t.kind {
                TagKind::Typed(v) => {
                    if let Some(tm) = v.get::<Timing>() {
                        self.start(*tm);
                    } else if v.get::<Restart>().is_some() {
                        self.st = None;
                        self.forget_before(t.at);
                    }
                }
                TagKind::Retuned { .. } => {
                    self.st = None;
                    self.forget_before(t.at);
                }
                _ => {}
            }
        }
        let mut out = std::mem::take(io.items_out::<SymbolBlock>(0));
        if let Some(mut t) = self.st.take() {
            let mut keep = true;
            if t.s < self.base + 8 {
                t.s = self.base + 8;
            }
            while t.s + lookahead(&t) <= self.end && t.s >= self.base + (3 * t.g + 16) as u64 {
                if !self.symbol(&mut t, &mut out) {
                    keep = false;
                    break;
                }
            }
            // A start before the buffered data (a backlog that was trimmed): move on by whole periods.
            while keep && t.s < self.base + (3 * t.g + 16) as u64 {
                t.s += t.p as u64;
            }
            if keep {
                self.st = Some(t);
            }
        }
        *io.items_out::<SymbolBlock>(0) = out;
        // Release old samples: keep what the finder can point back to, and two symbols before the current one.
        let keep_from = match &self.st {
            Some(t) => t.s.saturating_sub((2 * t.p + 3 * t.g) as u64),
            None => self.end.saturating_sub(120_000),
        }
        .min(self.end.saturating_sub(120_000).max(self.base));
        if keep_from > self.base + (1 << 18) {
            let d = (keep_from - self.base) as usize;
            self.buf.drain(..d);
            self.base = keep_from;
        }
        if let Ok(mut tl) = self.link.telemetry.lock() {
            tl.frac = self.frac();
            tl.timing = (self.last.0, self.last.1);
            tl.drift_ppm = if self.st.is_some() {
                self.drift_ppm
            } else {
                0.0
            };
            tl.resyncs = self.resyncs;
            tl.fft = self.st.as_ref().map(|t| (t.n, t.g));
            tl.now = self.end as f64 / self.rate;
        }
        let track = self.st.as_ref();
        self.p.put(&SyncFields {
            timing: track.map(|_| self.last.0),
            rho: track.map(|_| self.last.1),
            frac: track.map(|t| t.frac),
        });
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        match SyncSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SyncSetting::PeakOnly => self.peak_only = PEAK_ONLY.read(id, v)?,
        }
        Ok(Applied::Live)
    }
    fn values(&self) -> Values {
        values_of([(SyncSetting::PeakOnly, Value::Bool(self.peak_only))])
    }
    /// The timing and the count of resynchronisations belong to the signal of the frequency before.
    fn forget(&mut self, _: &Kept) {
        self.resyncs = 0;
        self.p.results_mut().fields_mut().clear();
    }
    fn results(&self) -> Option<&Results> {
        Some(self.p.results())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The search of a whole period and the echo profile use the room that the start of tracking made.
    #[test]
    fn a_search_and_the_echo_profile_do_not_grow_the_buffers() {
        let (n, g) = (2048, 512);
        let mut y = DvbtSync::default();
        y.buf = (0..6 * (n + g))
            .map(|i| Complex32::new((i as f32 * 0.37).sin(), (i as f32 * 0.11).cos()))
            .collect();
        y.end = y.buf.len() as u64;
        y.start(Timing {
            fft: n,
            guard: g,
            start: 2 * (n + g) as u64,
            frac: 0.0,
        });
        let mut t = y.st.take().expect("tracking started");
        let room = |y: &DvbtSync, t: &Track| {
            (
                y.corr.capacity(),
                t.profile.acc.capacity(),
                t.profile.spare.capacity(),
            )
        };
        let before = room(&y, &t);
        assert!(y.full_search(&t).is_some());
        // The profile is examined after 6 additions, then after 12 more.
        for _ in 0..20 {
            y.profile_add(&mut t, false);
        }
        t.profile.restart();
        y.profile_add(&mut t, false);
        assert_eq!(room(&y, &t), before);
    }

    /// The slope of the line fit is the drift of the boundaries, with the steps of the window in them.
    #[test]
    fn the_line_fit_reads_the_drift_through_window_steps() {
        let mut f = DriftFit::default();
        let (p, drift) = (2304usize, 0.05);
        let mut s = 10_000u64;
        for k in 0..500 {
            let true_pos = 10_000.0 + k as f64 * (p as f64 + drift);
            f.add(s, true_pos - s as f64, p);
            f.step();
            // The loop steps the window by a whole sample when the error passes one.
            s += p as u64 + u64::from(true_pos + p as f64 + drift - (s + p as u64) as f64 >= 1.0);
        }
        assert!((f.slope().expect("a slope") - drift).abs() < 1e-9);
    }
}
