//! The noise floor: a floor for one hop that signals do not lift, and a floor per bin of the grid that falls fast, rises slowly, ignores
//! signals that stay, and follows a gain change of a whole hop at once.

use crate::grid::{Grid, UNCOVERED_DB};

/// The floor of a hop's bins: their 20th percentile (bins never covered are left out). NaN for none.
pub fn hop_floor_db(bins_db: &[f32], scratch: &mut Vec<f32>) -> f32 {
    scratch.clear();
    scratch.extend(bins_db.iter().copied().filter(|&v| v > UNCOVERED_DB));
    if scratch.is_empty() {
        return f32::NAN;
    }
    let k = ((scratch.len() - 1) as f64 * 0.2).round() as usize;
    let (_, v, _) = scratch.select_nth_unstable_by(k, |a, b| a.total_cmp(b));
    *v
}

/// How the floor moves.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorConfig {
    /// The share of the way down the floor moves per sweep when a bin is below it.
    pub fall: f32,
    /// The share of the way up the floor moves per sweep when a bin is above it (and not a signal).
    pub rise: f32,
    /// A bin this far over the floor is a signal and does not raise it, dB.
    pub signal_db: f32,
    /// A whole hop that moves this far is a gain change, followed at once, dB.
    pub gain_step_db: f32,
    /// The local level of a bin is the median of the bins this far either side.
    pub window: usize,
}

impl Default for FloorConfig {
    fn default() -> Self {
        FloorConfig {
            fall: 0.3,
            rise: 0.05,
            signal_db: 6.0,
            gain_step_db: 3.0,
            window: 8,
        }
    }
}

/// A saved floor.
#[derive(Clone, Debug, PartialEq)]
pub struct FloorSnapshot {
    /// The grid it was measured on.
    pub grid: Grid,
    /// dBFS per bin ([`UNCOVERED_DB`] where unknown).
    pub floor_db: Vec<f32>,
}

impl FloorSnapshot {
    /// The bytes of the snapshot: the grid (low edge and bin width as f64, the bin count as u32), then one f32 per bin,
    /// little-endian.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(20 + 4 * self.floor_db.len());
        out.extend_from_slice(&self.grid.low_hz.to_le_bytes());
        out.extend_from_slice(&self.grid.bin_hz.to_le_bytes());
        out.extend_from_slice(
            &u32::try_from(self.grid.bins)
                .unwrap_or(u32::MAX)
                .to_le_bytes(),
        );
        for v in &self.floor_db {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }

    /// A snapshot from the bytes that [`FloorSnapshot::to_bytes`] returns. `None` if they do not hold a snapshot.
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        let f64_at = |i: usize| Some(f64::from_le_bytes(b.get(i..i + 8)?.try_into().ok()?));
        let low_hz = f64_at(0)?;
        let bin_hz = f64_at(8)?;
        let bins = u32::from_le_bytes(b.get(16..20)?.try_into().ok()?) as usize;
        let rest = b.get(20..)?;
        if bins == 0 || bins > crate::grid::MAX_BINS || rest.len() != 4 * bins {
            return None;
        }
        if !(low_hz.is_finite() && bin_hz.is_finite() && bin_hz > 0.0) {
            return None;
        }
        Some(FloorSnapshot {
            grid: Grid {
                low_hz,
                bin_hz,
                bins,
            },
            floor_db: rest
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
                .collect(),
        })
    }
}

/// One floor per grid bin.
#[derive(Debug)]
pub struct DynamicFloor {
    grid: Grid,
    cfg: FloorConfig,
    floor: Vec<f32>,
    scratch: Vec<f32>,
    scratch2: Vec<f32>,
    local: Vec<f32>,
    sweeps: u64,
}

impl DynamicFloor {
    /// An empty floor over `grid`.
    pub fn new(grid: Grid, cfg: FloorConfig) -> Self {
        DynamicFloor {
            floor: vec![f32::NAN; grid.bins],
            scratch: Vec::with_capacity(4096),
            scratch2: Vec::with_capacity(4096),
            local: vec![UNCOVERED_DB; grid.bins],
            grid,
            cfg,
            sweeps: 0,
        }
    }

    /// The floor, dBFS per bin (NaN where nothing was measured yet).
    pub fn floor_db(&self) -> &[f32] {
        &self.floor
    }

    /// Updates the floor with a stitched sweep. `hops` are the grid bin ranges each hop covered (for the gain-change rule and to
    /// start a bin's floor at the floor of its hop).
    pub fn update(&mut self, sweep_db: &[f32], hops: &[std::ops::Range<usize>]) {
        let n = self.grid.bins.min(sweep_db.len());
        // The local level: the median of the covered bins within the window.
        let w = self.cfg.window;
        for k in 0..n {
            if sweep_db[k] <= UNCOVERED_DB {
                self.local[k] = UNCOVERED_DB;
                continue;
            }
            self.scratch.clear();
            let (a, b) = (k.saturating_sub(w), (k + w + 1).min(n));
            self.scratch
                .extend(sweep_db[a..b].iter().copied().filter(|&v| v > UNCOVERED_DB));
            let m = self.scratch.len() / 2;
            let (_, v, _) = self
                .scratch
                .select_nth_unstable_by(m, |x, y| x.total_cmp(y));
            self.local[k] = *v;
        }
        for r in hops {
            let r = r.start.min(n)..r.end.min(n);
            if r.is_empty() {
                continue;
            }
            // Gain change of the whole hop.
            self.scratch.clear();
            for k in r.clone() {
                let (x, f) = (sweep_db[k], self.floor[k]);
                if x > UNCOVERED_DB && f.is_finite() {
                    self.scratch.push(x - f);
                }
            }
            if self.scratch.len() * 2 > r.len() {
                let m = crate::median(&self.scratch, &mut self.scratch2);
                if m.abs() > self.cfg.gain_step_db {
                    for f in &mut self.floor[r.clone()] {
                        if f.is_finite() {
                            *f += m;
                        }
                    }
                }
            }
            // New bins start at the floor of the hop, or at their local level if it is lower.
            if self.floor[r.clone()].iter().any(|f| !f.is_finite()) {
                let hf = hop_floor_db(&sweep_db[r.clone()], &mut self.scratch);
                if hf.is_finite() {
                    for k in r.clone() {
                        if !self.floor[k].is_finite() && sweep_db[k] > UNCOVERED_DB {
                            self.floor[k] = hf.min(self.local[k]);
                        }
                    }
                }
            }
        }
        for k in 0..n {
            let x = self.local[k];
            let f = self.floor[k];
            if x <= UNCOVERED_DB {
                continue;
            }
            if !f.is_finite() {
                self.floor[k] = x;
            } else if x < f {
                self.floor[k] = f + self.cfg.fall * (x - f);
            } else if x < f + self.cfg.signal_db {
                self.floor[k] = f + self.cfg.rise * (x - f);
            }
        }
        self.sweeps += 1;
    }

    /// Saves the floor.
    pub fn snapshot(&self) -> FloorSnapshot {
        FloorSnapshot {
            grid: self.grid,
            floor_db: self
                .floor
                .iter()
                .map(|&f| if f.is_finite() { f } else { UNCOVERED_DB })
                .collect(),
        }
    }

    /// Loads a saved floor. On the same grid it is copied. On another grid each bin takes the saved bin that holds its centre.
    pub fn load(&mut self, snap: &FloorSnapshot) {
        let known = |v: f32| if v > UNCOVERED_DB { v } else { f32::NAN };
        if self.grid.same_as(&snap.grid) && snap.floor_db.len() == self.floor.len() {
            for (f, &v) in self.floor.iter_mut().zip(&snap.floor_db) {
                *f = known(v);
            }
            return;
        }
        for k in 0..self.grid.bins {
            if let Some(j) = snap.grid.index_of(self.grid.centre_hz(k)) {
                if let Some(&v) = snap.floor_db.get(j) {
                    self.floor[k] = known(v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, level: f32, seed: u64) -> Vec<f32> {
        let mut r = crate::fake::Rng::new(seed);
        (0..n).map(|_| level + 2.0 * r.gauss() as f32).collect()
    }

    #[test]
    fn hop_floor_ignores_signals() {
        let mut v = noise(1000, -90.0, 1);
        for x in v.iter_mut().take(300) {
            *x = -40.0;
        }
        let f = hop_floor_db(&v, &mut Vec::new());
        assert!((f + 91.7).abs() < 1.5, "{f}");
    }

    #[test]
    fn falls_fast_rises_slowly_and_ignores_carriers() {
        let g = Grid::new(0.0, 1000.0, 1.0).unwrap();
        let hops = [0..500, 500..1000];
        let mut fl = DynamicFloor::new(g, FloorConfig::default());
        for s in 0..20 {
            let mut v = noise(1000, -90.0, s);
            v[100] = -40.0; // a carrier that stays
            for x in &mut v[700..740] {
                *x = -50.0; // a wide signal that stays
            }
            fl.update(&v, &hops);
        }
        assert!(fl.floor_db()[720] < -80.0, "{}", fl.floor_db()[720]);
        let f = fl.floor_db();
        assert!(f[100] < -85.0, "carrier not in the floor: {}", f[100]);
        assert!((f[300] + 90.5).abs() < 2.0, "{}", f[300]);
        // A quiet stretch: falls fast.
        let mut v = noise(1000, -90.0, 99);
        for x in &mut v[280..320] {
            *x = -110.0;
        }
        fl.update(&v, &hops);
        assert!(fl.floor_db()[300] < -95.0);
        // One low bin does not move it.
        let before = fl.floor_db()[600];
        let mut v = noise(1000, -90.0, 98);
        v[600] = -120.0;
        fl.update(&v, &hops);
        assert!((fl.floor_db()[600] - before).abs() < 1.0);
        // A small rise: rises slowly.
        let before = fl.floor_db()[400];
        let mut v = noise(1000, -90.0, 100);
        v[400] = before + 4.0;
        fl.update(&v, &hops);
        assert!(fl.floor_db()[400] - before < 0.5);
    }

    #[test]
    fn follows_a_gain_change_of_a_whole_hop() {
        let g = Grid::new(0.0, 1000.0, 1.0).unwrap();
        let hops = [0..500, 500..1000];
        let mut fl = DynamicFloor::new(g, FloorConfig::default());
        for s in 0..10 {
            fl.update(&noise(1000, -90.0, s), &hops);
        }
        let before = fl.floor_db()[700];
        let mut v = noise(1000, -90.0, 50);
        for x in &mut v[500..] {
            *x += 15.0;
        }
        fl.update(&v, &hops);
        assert!(fl.floor_db()[700] - before > 12.0, "followed");
        assert!(
            (fl.floor_db()[200] + 90.5).abs() < 2.0,
            "other hop untouched"
        );
    }

    #[test]
    fn saved_and_loaded_on_another_grid() {
        let g = Grid::new(0.0, 1000.0, 10.0).unwrap();
        let mut fl = DynamicFloor::new(g, FloorConfig::default());
        fl.update(&noise(100, -80.0, 3), &[0..100usize; 1]);
        let snap = fl.snapshot();
        let back = FloorSnapshot::from_bytes(&snap.to_bytes()).unwrap();
        assert_eq!(back, snap);
        assert_eq!(FloorSnapshot::from_bytes(&[0; 7]), None);
        let mut same = DynamicFloor::new(g, FloorConfig::default());
        same.load(&back);
        assert_eq!(same.floor_db(), fl.floor_db());
        let g2 = Grid::new(0.0, 1000.0, 5.0).unwrap();
        let mut other = DynamicFloor::new(g2, FloorConfig::default());
        other.load(&back);
        assert_eq!(other.floor_db()[3], fl.floor_db()[1]);
    }
}
