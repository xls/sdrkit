//! Synthetic ionograms of a layered ionosphere, for the scaling tests: parabolic layers, virtual heights from the group index, the
//! extraordinary trace, vertical and oblique echoes (Martyn's theorem over a spherical earth), noise and interfering carriers.

use super::scale::{col_hz, group_path_km, incidence_rad, IonogramDb, COLS, COL_HZ, ROWS, ROW_KM};
use sdrkit_algo_sim::rng::Rng;

/// A parabolic layer: critical frequency, peak height and half thickness.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The critical frequency, Hz.
    pub fc_hz: f64,
    /// The height of the peak, km.
    pub hm_km: f64,
    /// The half thickness, km.
    pub ym_km: f64,
}

impl Layer {
    /// The square of the plasma frequency at height `h_km`, Hz².
    pub fn fn2(&self, h_km: f64) -> f64 {
        let z = (h_km - self.hm_km) / self.ym_km;
        (self.fc_hz * self.fc_hz * (1.0 - z * z)).max(0.0)
    }
}

/// An ionosphere: its layers and the electron gyrofrequency.
#[derive(Clone, Debug, PartialEq)]
pub struct Ionosphere {
    /// The layers; the plasma frequency at a height is the highest of theirs.
    pub layers: Vec<Layer>,
    /// The gyrofrequency, Hz.
    pub fh_hz: f64,
}

impl Ionosphere {
    /// E (foE 3.0 MHz, hmE 110 km, ymE 20 km) and F2 (foF2 7.0 MHz, hmF2 320 km, ymF2 100 km), fH 1.3 MHz; with `f1`, an F1 layer
    /// (foF1 4.5 MHz, hmF1 200 km, ymF1 40 km).
    pub fn standard(f1: bool) -> Ionosphere {
        let mut layers = vec![
            Layer {
                fc_hz: 3.0e6,
                hm_km: 110.0,
                ym_km: 20.0,
            },
            Layer {
                fc_hz: 7.0e6,
                hm_km: 320.0,
                ym_km: 100.0,
            },
        ];
        if f1 {
            layers.push(Layer {
                fc_hz: 4.5e6,
                hm_km: 200.0,
                ym_km: 40.0,
            });
        }
        Ionosphere {
            layers,
            fh_hz: 1.3e6,
        }
    }

    fn fn2(&self, h: f64) -> f64 {
        self.layers.iter().map(|l| l.fn2(h)).fold(0.0, f64::max)
    }

    /// The virtual height of an ordinary echo at `f_hz`: the integral of the group index 1 / √(1 − fN²/f²) up to the reflection,
    /// km. `None` when the wave goes through every layer.
    pub fn virtual_height_km(&self, f_hz: f64) -> Option<f64> {
        let f2 = f_hz * f_hz;
        let dh = 0.05;
        let mut h = 0.0;
        let mut hv = 0.0;
        while h < 1000.0 {
            let x = self.fn2(h + dh) / f2;
            if x >= 0.98 {
                // The reflection height by bisection, then the tail with the plasma frequency taken as linear in height.
                let (mut a, mut b) = (h, h + dh);
                if self.fn2(b) / f2 < 1.0 {
                    b = h + dh;
                    while self.fn2(b) / f2 < 1.0 {
                        b += dh;
                        if b > 1000.0 {
                            return None;
                        }
                    }
                }
                for _ in 0..60 {
                    let m = 0.5 * (a + b);
                    if self.fn2(m) / f2 >= 1.0 {
                        b = m;
                    } else {
                        a = m;
                    }
                }
                let x0 = self.fn2(h) / f2;
                return Some(hv + 2.0 * (b - h) / (1.0 - x0).max(1e-9).sqrt());
            }
            let xm = self.fn2(h + 0.5 * dh) / f2;
            hv += dh / (1.0 - xm).sqrt();
            h += dh;
        }
        None
    }

    /// The virtual height of an extraordinary echo at `f_hz`: the ordinary height at √(f (f − fH)), km.
    pub fn virtual_height_x_km(&self, f_hz: f64) -> Option<f64> {
        if f_hz <= self.fh_hz {
            return None;
        }
        self.virtual_height_km((f_hz * (f_hz - self.fh_hz)).sqrt())
    }
}

/// What a synthetic ionogram holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// The ionosphere.
    pub iono: Ionosphere,
    /// The swept frequencies, Hz.
    pub swept_hz: (f64, f64),
    /// The ground distance of an oblique path, km; `None` for a vertical ionogram.
    pub distance_km: Option<f64>,
    /// The most hops drawn.
    pub hops: u32,
    /// Interfering carriers, Hz.
    pub carriers_hz: Vec<f64>,
    /// No echoes, only noise.
    pub noise_only: bool,
    /// The noise seed.
    pub seed: u64,
}

impl Scene {
    /// A vertical ionogram swept from 1 to 20 MHz.
    pub fn vertical(f1: bool) -> Scene {
        Scene {
            iono: Ionosphere::standard(f1),
            swept_hz: (1e6, 20e6),
            distance_km: None,
            hops: 2,
            carriers_hz: Vec::new(),
            noise_only: false,
            seed: 1,
        }
    }
    /// An oblique ionogram over `distance_km`, swept from 1 to 30 MHz, with up to two hops.
    pub fn oblique(distance_km: f64) -> Scene {
        Scene {
            distance_km: Some(distance_km),
            swept_hz: (1e6, 30e6),
            ..Scene::vertical(false)
        }
    }
}

/// Echoes stand this far over the noise, dB.
const ECHO_DB: f64 = 15.0;
/// The extraordinary echo is this much weaker, dB.
const X_DB: f64 = -5.0;
/// Each further hop is this much weaker, dB.
const HOP_DB: f64 = -6.0;

/// Makes the ionogram of `scene` in dB over each column's median: noise of 3 averaged exponential powers per cell, echoes 15 dB
/// over it.
pub fn ionogram(scene: &Scene) -> IonogramDb {
    let mut rng = Rng::new(scene.seed);
    let mut p = vec![0.0f64; ROWS * COLS];
    let mut swept = vec![false; COLS];
    for (col, s) in swept.iter_mut().enumerate() {
        let f = col_hz(col);
        *s = f >= scene.swept_hz.0 && f <= scene.swept_hz.1;
    }
    for v in p.iter_mut() {
        *v = (0..3).map(|_| -rng.uniform_open().ln()).sum::<f64>() / 3.0;
    }
    let add = |p: &mut Vec<f64>, col: usize, range_km: f64, db: f64| {
        let row = (range_km / ROW_KM).round();
        if row >= 0.0 && (row as usize) < ROWS && swept[col] {
            p[row as usize * COLS + col] += 10f64.powf(db / 10.0);
        }
    };
    if !scene.noise_only {
        match scene.distance_km {
            None => {
                for col in 0..COLS {
                    let f = col_hz(col);
                    add(&mut p, col, 0.0, ECHO_DB);
                    for (h, extra) in [
                        (scene.iono.virtual_height_km(f), 0.0),
                        (scene.iono.virtual_height_x_km(f), X_DB),
                    ] {
                        let Some(h) = h else { continue };
                        for hop in 1..=scene.hops {
                            let db = ECHO_DB + extra + HOP_DB * (hop - 1) as f64;
                            add(&mut p, col, 2.0 * hop as f64 * h, db);
                        }
                    }
                }
            }
            Some(d) => {
                // Martyn's theorem: an oblique echo at f has the virtual height of the vertical echo at f cos φ.
                // The E layer screens the F layer below its own MUF of as many hops.
                let step = COL_HZ / 8.0;
                let fo_e = scene.iono.layers.first().map_or(0.0, |l| l.fc_hz);
                for hop in 1..=scene.hops {
                    let mut e_muf = 0.0f64;
                    let mut fv = 0.5e6;
                    while fv < 15e6 {
                        if let Some(h) = scene.iono.virtual_height_km(fv) {
                            let phi = incidence_rad(hop, d, h);
                            let f = fv / phi.cos();
                            if fv < fo_e {
                                e_muf = e_muf.max(f);
                            }
                            let col = (f / COL_HZ) as usize;
                            if col < COLS && (fv < fo_e || f > e_muf) {
                                let path = group_path_km(hop, d, h);
                                let db = ECHO_DB + HOP_DB * (hop - 1) as f64;
                                let row = (path / ROW_KM).round() as usize;
                                if row < ROWS && swept[col] && p[row * COLS + col] < 10.0 {
                                    p[row * COLS + col] += 10f64.powf(db / 10.0);
                                }
                            }
                        }
                        fv += step;
                    }
                }
            }
        }
    }
    for &c in &scene.carriers_hz {
        let col = (c / COL_HZ) as usize;
        if col < COLS {
            for row in 0..ROWS {
                p[row * COLS + col] += 100.0;
            }
        }
    }
    let mut db = vec![0.0f32; ROWS * COLS];
    let mut scratch = Vec::with_capacity(ROWS);
    for col in 0..COLS {
        scratch.clear();
        scratch.extend((0..ROWS).map(|r| p[r * COLS + col]));
        let med = sdrkit_algo_dsp::stats::quantile_f64(&scratch.clone(), 0.5, &mut scratch)
            .unwrap_or(1.0)
            .max(1e-30);
        for row in 0..ROWS {
            db[row * COLS + col] = (10.0 * (p[row * COLS + col] / med).log10()) as f32;
        }
    }
    IonogramDb { db, swept }
}
