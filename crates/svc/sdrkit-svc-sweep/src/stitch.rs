//! Hops onto the grid: the kept part of each hop without its DC bins, cross-faded where hops overlap, with the shape that
//! every hop shares learnt and removed, and spurs at hop centres removed.

use crate::grid::{Grid, UNCOVERED_DB};
use crate::{db, lin, median};

/// How hops are stitched.
#[derive(Clone, Debug, PartialEq)]
pub struct StitchConfig {
    /// The FFT bins dropped at DC: the DC bin and `dc_bins - 1` either side, none for 0 (`None`: `max(5, fft / 512)`, the main lobe of the Blackman-Harris window).
    pub dc_bins: Option<usize>,
    /// Offsets from the centre closer than this are not used, Hz (a radio's own sweep keeps 0.125–0.375 × rate either side).
    pub inner_keep_hz: f64,
    /// Learn and remove the hop shape.
    pub flatten: bool,
    /// Remove spurs at hop centres.
    pub remove_spurs: bool,
}

impl Default for StitchConfig {
    fn default() -> Self {
        StitchConfig {
            dc_bins: None,
            inner_keep_hz: 0.0,
            flatten: true,
            remove_spurs: true,
        }
    }
}

const SHAPE_ROWS: usize = 64;
const SHAPE_SEGS: usize = 128;

/// Learns the shape every hop shares.
#[derive(Debug)]
struct ShapeLearner {
    n: usize,
    segs: usize,
    rows: Vec<f32>,
    count: usize,
    next: usize,
    seg_db: Vec<f32>,
    shape_seg: Vec<f32>,
    /// Per FFT bin, the linear gain to divide out.
    gain: Vec<f32>,
    scratch: Vec<f32>,
    col: Vec<f32>,
    since: usize,
}

impl ShapeLearner {
    fn new(n: usize) -> Self {
        let segs = SHAPE_SEGS.min(n);
        ShapeLearner {
            n,
            segs,
            rows: vec![0.0; SHAPE_ROWS * segs],
            count: 0,
            next: 0,
            seg_db: vec![0.0; segs],
            shape_seg: vec![0.0; segs],
            gain: vec![1.0; n],
            scratch: Vec::with_capacity(SHAPE_ROWS.max(segs)),
            col: Vec::with_capacity(SHAPE_ROWS.max(segs)),
            since: 0,
        }
    }

    fn observe(&mut self, power: &[f32], dc_lo: usize, dc_hi: usize) {
        let per = self.n / self.segs;
        for s in 0..self.segs {
            let (a, b) = (s * per, (s + 1) * per);
            let mut sum = 0.0f64;
            let mut k = 0usize;
            for (i, &p) in power.iter().enumerate().take(b).skip(a) {
                if i < dc_lo || i >= dc_hi {
                    sum += p as f64;
                    k += 1;
                }
            }
            self.seg_db[s] = if k > 0 {
                db((sum / k as f64) as f32)
            } else {
                f32::NAN
            };
        }
        let finite: usize = self.seg_db.iter().filter(|v| v.is_finite()).count();
        if finite == 0 {
            return;
        }
        self.scratch.clear();
        self.scratch
            .extend(self.seg_db.iter().copied().filter(|v| v.is_finite()));
        let m = {
            let k = self.scratch.len() / 2;
            let (_, v, _) = self
                .scratch
                .select_nth_unstable_by(k, |a, b| a.total_cmp(b));
            *v
        };
        let row = &mut self.rows[self.next * self.segs..(self.next + 1) * self.segs];
        for (r, &d) in row.iter_mut().zip(&self.seg_db) {
            *r = d - m;
        }
        self.next = (self.next + 1) % SHAPE_ROWS;
        self.count = (self.count + 1).min(SHAPE_ROWS);
        self.since += 1;
        if self.count >= 3 && (self.since >= 16 || self.count <= 16) {
            self.recompute();
        }
    }

    fn recompute(&mut self) {
        self.since = 0;
        for s in 0..self.segs {
            self.col.clear();
            for r in 0..self.count {
                let v = self.rows[r * self.segs + s];
                if v.is_finite() {
                    self.col.push(v);
                }
            }
            self.shape_seg[s] = if self.col.is_empty() {
                f32::NAN
            } else {
                median(&self.col, &mut self.scratch)
            };
        }
        // Segments with no data (DC) take their neighbours' value.
        for s in 0..self.segs {
            if !self.shape_seg[s].is_finite() {
                let l = (0..s).rev().find(|&j| self.shape_seg[j].is_finite());
                let r = (s + 1..self.segs).find(|&j| self.shape_seg[j].is_finite());
                self.shape_seg[s] = match (l, r) {
                    (Some(l), Some(r)) => (self.shape_seg[l] + self.shape_seg[r]) / 2.0,
                    (Some(l), None) => self.shape_seg[l],
                    (None, Some(r)) => self.shape_seg[r],
                    (None, None) => 0.0,
                };
            }
        }
        // The shape is smooth: a running median over seven segments keeps narrow features (a spur at a fixed offset from the tuning,
        // which is flagged as such later) out of it.
        self.col.clear();
        self.col.extend_from_slice(&self.shape_seg);
        for s in 0..self.segs {
            let a = s.saturating_sub(3);
            let b = (s + 4).min(self.segs);
            self.shape_seg[s] = median(&self.col[a..b], &mut self.scratch);
        }
        // The middle half sets 0 dB.
        let (a, b) = (self.segs / 4, 3 * self.segs / 4);
        self.scratch.clear();
        self.scratch
            .extend_from_slice(&self.shape_seg[a..b.max(a + 1)]);
        let k = self.scratch.len() / 2;
        let refv = {
            let (_, v, _) = self
                .scratch
                .select_nth_unstable_by(k, |x, y| x.total_cmp(y));
            *v
        };
        for v in &mut self.shape_seg {
            *v -= refv;
        }
        let per = (self.n / self.segs) as f32;
        for i in 0..self.n {
            // Linear between segment centres.
            let x = (i as f32 + 0.5) / per - 0.5;
            let s0 = x.floor().clamp(0.0, (self.segs - 1) as f32) as usize;
            let s1 = (s0 + 1).min(self.segs - 1);
            let t = (x - s0 as f32).clamp(0.0, 1.0);
            let d = self.shape_seg[s0] * (1.0 - t) + self.shape_seg[s1] * t;
            self.gain[i] = lin(d);
        }
    }
}

/// Stitches hops onto a grid.
#[derive(Debug)]
pub struct Stitcher {
    grid: Grid,
    cfg: StitchConfig,
    n: usize,
    rate_hz: f64,
    dc_bins: usize,
    acc: Vec<f64>,
    wsum: Vec<f32>,
    dc_only: Vec<bool>,
    db: Vec<f32>,
    centres: Vec<f64>,
    shape: ShapeLearner,
    scratch: Vec<f32>,
    scratch2: Vec<f32>,
    hops: usize,
}

impl Stitcher {
    /// A stitcher for hops of `fft_size` bins at `rate_hz` onto `grid`.
    pub fn new(grid: Grid, fft_size: usize, rate_hz: f64, cfg: StitchConfig) -> Self {
        let dc_bins = cfg.dc_bins.unwrap_or((fft_size / 512).max(5));
        Stitcher {
            acc: vec![0.0; grid.bins],
            wsum: vec![0.0; grid.bins],
            dc_only: vec![false; grid.bins],
            db: vec![UNCOVERED_DB; grid.bins],
            centres: Vec::with_capacity(256),
            shape: ShapeLearner::new(fft_size),
            scratch: Vec::with_capacity(64),
            scratch2: Vec::with_capacity(64),
            grid,
            cfg,
            n: fft_size,
            rate_hz,
            dc_bins,
            hops: 0,
        }
    }

    /// Makes room for this many hops per sweep (so a sweep allocates nothing).
    pub fn reserve_hops(&mut self, hops: usize) {
        self.centres
            .reserve(hops.saturating_sub(self.centres.len()));
    }

    /// Starts a sweep.
    pub fn begin_sweep(&mut self) {
        self.acc.iter_mut().for_each(|v| *v = 0.0);
        self.wsum.iter_mut().for_each(|v| *v = 0.0);
        self.dc_only.iter_mut().for_each(|v| *v = false);
        self.centres.clear();
        self.hops = 0;
    }

    #[cfg(test)]
    /// The hop shape now divided out, dB per FFT bin (0 at the centre when flat).
    pub fn shape_db(&self) -> Vec<f32> {
        self.shape.gain.iter().map(|&g| db(g)).collect()
    }

    /// Adds a hop: `power` is the averaged FFT (linear, lowest frequency first, DC at `len / 2`), captured at `centre_hz`. Only
    /// `keep_lo_hz ..= keep_hi_hz` is used.
    pub fn add_hop(&mut self, centre_hz: f64, keep_lo_hz: f64, keep_hi_hz: f64, power: &[f32]) {
        let n = self.n;
        if power.len() != n {
            return;
        }
        let half = n / 2;
        // The DC bin and `dc_bins - 1` either side (none for 0): `dc_lo..dc_hi`.
        let (dc_lo, dc_hi) = if self.dc_bins == 0 {
            (half, half)
        } else {
            (half + 1 - self.dc_bins.min(half), half + self.dc_bins)
        };
        if self.cfg.flatten {
            self.shape.observe(power, dc_lo, dc_hi);
        }
        let fbin = self.rate_hz / n as f64;
        let g = self.grid;
        let half_keep = (centre_hz - keep_lo_hz)
            .max(keep_hi_hz - centre_hz)
            .max(fbin);
        let range = g.bins_between(keep_lo_hz, keep_hi_hz);
        for k in range {
            let fk = g.centre_hz(k);
            let off = fk - centre_hz;
            if off.abs() < self.cfg.inner_keep_hz {
                continue;
            }
            let x_lo = (off - g.bin_hz / 2.0) / fbin + half as f64;
            let x_hi = (off + g.bin_hz / 2.0) / fbin + half as f64;
            let mut i_lo = x_lo.ceil().max(0.0) as usize;
            let mut i_hi = (x_hi.ceil().max(0.0) as usize).min(n);
            if i_lo >= i_hi {
                let i = (off / fbin + half as f64).round();
                if i < 0.0 || i >= n as f64 {
                    continue;
                }
                i_lo = i as usize;
                i_hi = i_lo + 1;
            }
            let mut sum = 0.0f64;
            let mut cnt = 0usize;
            let mut at_dc = false;
            for (i, &p) in power.iter().enumerate().take(i_hi).skip(i_lo) {
                if i >= dc_lo && i < dc_hi {
                    at_dc = true;
                    continue;
                }
                sum += (p / self.shape.gain[i]) as f64;
                cnt += 1;
            }
            if cnt == 0 {
                if at_dc {
                    self.dc_only[k] = true;
                }
                continue;
            }
            let w = (1.0 - 0.9 * (off.abs() / half_keep).min(1.0)) as f32;
            self.acc[k] += w as f64 * sum / cnt as f64;
            self.wsum[k] += w;
        }
        self.centres.push(centre_hz);
        self.hops += 1;
    }

    /// Ends the sweep and returns the stitched dBFS per grid bin.
    pub fn finish_sweep(&mut self) -> &[f32] {
        for k in 0..self.grid.bins {
            self.db[k] = if self.wsum[k] > 0.0 {
                db((self.acc[k] / self.wsum[k] as f64) as f32)
            } else {
                UNCOVERED_DB
            };
        }
        // Fill the DC gaps from their neighbours.
        let mut k = 0;
        while k < self.grid.bins {
            if self.wsum[k] == 0.0 && self.dc_only[k] {
                let start = k;
                while k < self.grid.bins && self.wsum[k] == 0.0 && self.dc_only[k] {
                    k += 1;
                }
                let left = (start > 0 && self.wsum[start - 1] > 0.0).then(|| self.db[start - 1]);
                let right = (k < self.grid.bins && self.wsum[k] > 0.0).then(|| self.db[k]);
                let len = (k - start) as f32;
                for (j, d) in self.db[start..k].iter_mut().enumerate() {
                    *d = match (left, right) {
                        (Some(l), Some(r)) => l + (r - l) * (j as f32 + 1.0) / (len + 1.0),
                        (Some(l), None) => l,
                        (None, Some(r)) => r,
                        (None, None) => UNCOVERED_DB,
                    };
                }
            } else {
                k += 1;
            }
        }
        if self.cfg.remove_spurs {
            self.remove_spurs();
        }
        &self.db
    }

    fn remove_spurs(&mut self) {
        let g = self.grid;
        let fbin = self.rate_hz / self.n as f64;
        let width = g.bin_hz.max(3.0 * fbin);
        for ci in 0..self.centres.len() {
            let c = self.centres[ci];
            let inside = g.bins_between(c - width, c + width);
            if inside.is_empty() {
                continue;
            }
            let span = inside.len().clamp(1, 4);
            self.scratch.clear();
            let lo_start = inside.start.saturating_sub(span);
            for k in lo_start..inside.start {
                if self.db[k] > UNCOVERED_DB {
                    self.scratch.push(self.db[k]);
                }
            }
            for k in inside.end..(inside.end + span).min(g.bins) {
                if self.db[k] > UNCOVERED_DB {
                    self.scratch.push(self.db[k]);
                }
            }
            if self.scratch.is_empty() {
                continue;
            }
            let m = median(&self.scratch, &mut self.scratch2);
            for k in inside {
                if self.db[k] > UNCOVERED_DB && self.db[k] > m + 3.0 {
                    self.db[k] = m;
                }
            }
        }
    }

    /// Changes which offsets from the centre are dropped (a radio's own sweep drops the middle of each block).
    pub fn set_inner_keep_hz(&mut self, hz: f64) {
        self.cfg.inner_keep_hz = hz.max(0.0);
    }

    /// The level of grid bin `k` from the hops added so far in this sweep, dBFS ([`UNCOVERED_DB`] when none covered it).
    pub fn partial_db(&self, k: usize) -> f32 {
        match self.wsum.get(k) {
            Some(&w) if w > 0.0 => db((self.acc[k] / w as f64) as f32),
            _ => UNCOVERED_DB,
        }
    }

    /// The FFT size of the hops.
    pub fn fft_size(&self) -> usize {
        self.n
    }

    /// The rate of the hops, Hz.
    pub fn rate_hz(&self) -> f64 {
        self.rate_hz
    }

    /// Hops added in this sweep.
    pub fn hops(&self) -> usize {
        self.hops
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(n: usize, db_level: f32) -> Vec<f32> {
        vec![lin(db_level); n]
    }

    fn no_shape() -> StitchConfig {
        StitchConfig {
            flatten: false,
            remove_spurs: false,
            ..Default::default()
        }
    }

    #[test]
    fn covers_every_bin_and_keeps_edge_hops() {
        // 100-108 MHz at 100 kHz, hops of 2 MHz kept 1.5 MHz.
        let g = Grid::new(100e6, 108e6, 100e3).unwrap();
        let n = 256;
        let rate = 2e6;
        let mut s = Stitcher::new(g, n, rate, no_shape());
        s.begin_sweep();
        let plan =
            crate::plan::HopPlan::new(&crate::plan::PlanConfig::new(100e6, 108e6, rate)).unwrap();
        for h in &plan.hops {
            let mut p = flat(n, -80.0);
            // A tone at 107.95 MHz (-30 dB) and at 104.0 MHz.
            for (f, l) in [(107.95e6, -30.0), (104.03e6, -40.0)] {
                let i = ((f - h.centre_hz) / (rate / n as f64) + (n / 2) as f64).round();
                if i >= 0.0 && (i as usize) < n && f >= h.keep_lo_hz && f <= h.keep_hi_hz {
                    p[i as usize] = lin(l);
                }
            }
            s.add_hop(h.centre_hz, h.keep_lo_hz, h.keep_hi_hz, &p);
        }
        let d = s.finish_sweep().to_vec();
        assert!(d.iter().all(|&v| v > -100.0), "every bin covered");
        // A grid bin of 100 kHz holds 12.8 FFT bins: the tone's power is averaged over them.
        let k = g.index_of(107.95e6).unwrap();
        assert!(d[k] > -50.0 && d[k] < -30.0, "edge tone {}", d[k]);
        assert!(d[k] > d[k - 1] + 10.0);
        let k = g.index_of(104.03e6).unwrap();
        assert!(d[k] > d[k + 1] + 10.0, "tone in its own bin");
    }

    #[test]
    fn uncovered_bins_and_dc_gaps() {
        let g = Grid::new(100e6, 104e6, 10e3).unwrap();
        let n = 1024;
        let rate = 1.024e6;
        let mut s = Stitcher::new(g, n, rate, no_shape());
        s.begin_sweep();
        // One hop at 101 MHz, kept 100.6-101.4: the rest is uncovered.
        let mut p = flat(n, -70.0);
        p[n / 2] = lin(-10.0); // DC spike
        s.add_hop(101e6, 100.6e6, 101.4e6, &p);
        let d = s.finish_sweep();
        assert_eq!(d[g.index_of(100.1e6).unwrap()], UNCOVERED_DB);
        assert_eq!(d[g.index_of(103e6).unwrap()], UNCOVERED_DB);
        let k = g.index_of(101e6).unwrap();
        assert!((d[k] + 70.0).abs() < 0.5, "DC removed and filled: {}", d[k]);
    }

    #[test]
    fn a_coarse_fft_on_a_fine_grid_still_covers() {
        let g = Grid::new(100e6, 101e6, 1e3).unwrap();
        let n = 256;
        let rate = 1.024e6; // 4 kHz FFT bins
        let mut s = Stitcher::new(g, n, rate, no_shape());
        s.begin_sweep();
        s.add_hop(100.5e6, 100e6, 101e6, &flat(n, -60.0));
        let d = s.finish_sweep();
        assert!(d.iter().all(|&v| (v + 60.0).abs() < 0.01));
    }

    #[test]
    fn overlap_blends_and_flatness_is_learnt() {
        let n = 512;
        let rate = 2e6;
        let g = Grid::new(100e6, 120e6, 20e3).unwrap();
        let cfg = StitchConfig {
            remove_spurs: false,
            ..Default::default()
        };
        let mut s = Stitcher::new(g, n, rate, cfg);
        let plan =
            crate::plan::HopPlan::new(&crate::plan::PlanConfig::new(100e6, 120e6, rate)).unwrap();
        // Every hop: a -80 dB floor with a roll-off of up to 6 dB at the edges, and a carrier somewhere.
        let shaped = |i: usize| {
            let x = (i as f32 - (n / 2) as f32) / (n / 2) as f32;
            -80.0 - 6.0 * x * x
        };
        let mut last = Vec::new();
        for sweep in 0..2 {
            s.begin_sweep();
            for h in &plan.hops {
                let mut p: Vec<f32> = (0..n).map(|i| lin(shaped(i))).collect();
                p[(h.index * 37) % n] = lin(-40.0);
                s.add_hop(h.centre_hz, h.keep_lo_hz, h.keep_hi_hz, &p);
            }
            last = s.finish_sweep().to_vec();
            if sweep == 0 {
                continue;
            }
        }
        let mut sorted = last.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let med = sorted[sorted.len() / 2];
        let p10 = sorted[sorted.len() / 10];
        assert!((med + 80.0).abs() < 1.0, "median {med}");
        assert!(
            med - p10 < 1.0,
            "flat after correction: p10 {p10} median {med}"
        );
        let shape = s.shape_db();
        assert!(shape[n / 2].abs() < 0.5);
        assert!(shape[n / 8] < -2.0, "edge shape learnt {}", shape[n / 8]);
    }

    #[test]
    fn spurs_at_hop_centres_are_removed() {
        let g = Grid::new(100e6, 102e6, 50e3).unwrap();
        let n = 256;
        let rate = 1e6;
        let mut cfg = no_shape();
        cfg.remove_spurs = true;
        cfg.dc_bins = Some(0);
        let mut s = Stitcher::new(g, n, rate, cfg);
        s.begin_sweep();
        let mut p = flat(n, -70.0);
        p[n / 2] = lin(-20.0); // a spur right at the centre
        p[n / 2 + 60] = lin(-20.0); // a real carrier 234 kHz away
        s.add_hop(101e6, 100.6e6, 101.4e6, &p);
        let before: Vec<f32> = {
            let mut c2 = no_shape();
            c2.dc_bins = Some(0);
            let mut t = Stitcher::new(g, n, rate, c2);
            t.begin_sweep();
            t.add_hop(101e6, 100.6e6, 101.4e6, &p);
            t.finish_sweep().to_vec()
        };
        let d = s.finish_sweep().to_vec();
        let changed = before
            .iter()
            .zip(&d)
            .filter(|(a, b)| (*a - *b).abs() > 0.01)
            .count();
        assert!((1..=2).contains(&changed), "changed {changed}");
        let kc = g.index_of(101e6 + 60.0 * rate / n as f64).unwrap();
        assert_eq!(d[kc], before[kc], "real carrier kept");
    }
}
