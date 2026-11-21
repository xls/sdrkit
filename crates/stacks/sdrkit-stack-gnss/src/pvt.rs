//! The position, velocity and time solver: iterative weighted least squares with one clock unknown for each clock group, the
//! earth's rotation during the travel time, Klobuchar and Saastamoinen corrections, the elevation mask, a residual check that
//! leaves out a bad satellite, DOP and error estimates, and the velocity from the Dopplers.

use crate::geo::{az_el, ecef_to_enu, ecef_to_geodetic, saastamoinen, Klobuchar};
use crate::system::{SatId, C, L1_HZ, OMEGA_E};

/// Inverts a 4×4 matrix.
pub fn invert4(m: &[[f64; 4]; 4]) -> Option<[[f64; 4]; 4]> {
    let v: Vec<Vec<f64>> = m.iter().map(|r| r.to_vec()).collect();
    let inv = invert(&v)?;
    let mut o = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            o[i][j] = inv[i][j];
        }
    }
    Some(o)
}

/// Inverts an n×n matrix by Gauss-Jordan elimination with partial pivoting.
pub fn invert(m: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = m.len();
    let mut a: Vec<Vec<f64>> = m
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut row = r.clone();
            row.extend((0..n).map(|j| if i == j { 1.0 } else { 0.0 }));
            row
        })
        .collect();
    for c in 0..n {
        let p = (c..n).max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))?;
        if a[p][c].abs() < 1e-14 {
            return None;
        }
        a.swap(c, p);
        let d = a[c][c];
        for v in a[c].iter_mut() {
            *v /= d;
        }
        for r in 0..n {
            if r != c {
                let f = a[r][c];
                if f != 0.0 {
                    for k in 0..2 * n {
                        a[r][k] -= f * a[c][k];
                    }
                }
            }
        }
    }
    Some(a.into_iter().map(|r| r[n..].to_vec()).collect())
}

/// One satellite's measurement for the solver.
#[derive(Clone, Debug)]
pub struct Obs {
    pub sat: SatId,
    /// Clock group: the index of the clock unknown it shares (GPS-scale systems share 0).
    pub group: usize,
    /// ECEF at the transmit instant (earth-fixed frame of that instant).
    pub pos: [f64; 3],
    pub vel: [f64; 3],
    /// Pseudorange corrected for the satellite clock and group delay, m.
    pub pr: f64,
    /// C/N0 weight.
    pub weight: f64,
    pub doppler: f64,
    pub carrier: f64,
    /// Satellite clock drift, s/s.
    pub clock_drift: f64,
}

/// Solver options.
#[derive(Clone, Debug)]
pub struct SolveOptions {
    pub mask_deg: f64,
    pub klobuchar: Option<Klobuchar>,
    pub gps_tow: f64,
    pub prior: Option<[f64; 3]>,
    pub raim: bool,
    pub iono: bool,
    pub tropo: bool,
}

impl Default for SolveOptions {
    fn default() -> Self {
        SolveOptions {
            mask_deg: 5.0,
            klobuchar: None,
            gps_tow: 0.0,
            prior: None,
            raim: true,
            iono: true,
            tropo: true,
        }
    }
}

/// A solution.
#[derive(Clone, Debug, Default)]
pub struct Solution {
    pub pos: [f64; 3],
    pub geo: [f64; 3],
    /// Clock bias (m) per group, `None` for a group not in the solution.
    pub clocks: Vec<Option<f64>>,
    pub hdop: f64,
    pub vdop: f64,
    pub pdop: f64,
    pub tdop: f64,
    pub herr: f64,
    pub verr: f64,
    pub rms: f64,
    /// Satellites used, with residual (m), azimuth and elevation (radians).
    pub used: Vec<(SatId, f64, f64, f64)>,
    pub excluded: usize,
    pub iterations: usize,
}

/// Chi-square limit at false alarm 0.001 for `k` degrees of freedom (Wilson-Hilferty).
pub fn chi2_limit(k: usize) -> f64 {
    let k = k as f64;
    let a = 2.0 / (9.0 * k);
    k * (1.0 - a + 3.09 * a.sqrt()).powi(3)
}

struct Lsq {
    pos: [f64; 3],
    clocks: Vec<f64>,
    groups: Vec<usize>,
    resid: Vec<f64>,
    w: Vec<f64>,
    h: Vec<[f64; 3]>,
    azel: Vec<(f64, f64)>,
    iterations: usize,
}

fn rotate(p: [f64; 3], tau: f64) -> [f64; 3] {
    let th = OMEGA_E * tau;
    let (s, c) = th.sin_cos();
    [c * p[0] + s * p[1], -s * p[0] + c * p[1], p[2]]
}

/// Least squares over `obs` (indices `use_idx`), starting from `start`.
fn lsq(obs: &[Obs], use_idx: &[usize], start: [f64; 3], opt: &SolveOptions) -> Option<Lsq> {
    let mut groups: Vec<usize> = use_idx.iter().map(|&i| obs[i].group).collect();
    groups.sort();
    groups.dedup();
    let ng = groups.len();
    let nu = 3 + ng;
    if use_idx.len() < nu || use_idx.len() < 4 {
        return None;
    }
    let mut pos = start;
    let mut clk = vec![0.0; ng];
    let mut iterations = 0;
    let mut out_h = Vec::new();
    let mut out_r = Vec::new();
    let mut out_w = Vec::new();
    let mut out_ae = Vec::new();
    for it in 0..20 {
        iterations = it + 1;
        let have_pos = (pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]).sqrt() > 1e6;
        let g = if have_pos {
            ecef_to_geodetic(pos)
        } else {
            [0.0; 3]
        };
        let mut ata = vec![vec![0.0; nu]; nu];
        let mut atb = vec![0.0; nu];
        out_h.clear();
        out_r.clear();
        out_w.clear();
        out_ae.clear();
        for &i in use_idx {
            let o = &obs[i];
            let tau0 = if have_pos {
                crate::geo::dist(o.pos, pos) / C
            } else {
                0.075
            };
            let sp = rotate(o.pos, tau0);
            let r = crate::geo::dist(sp, pos);
            let e = [
                (sp[0] - pos[0]) / r,
                (sp[1] - pos[1]) / r,
                (sp[2] - pos[2]) / r,
            ];
            let (az, el) = if have_pos {
                az_el(g, pos, sp)
            } else {
                (0.0, std::f64::consts::FRAC_PI_2)
            };
            let mut corr = 0.0;
            if have_pos && el > 0.5f64.to_radians() {
                if opt.iono {
                    if let Some(k) = &opt.klobuchar {
                        corr += k.delay_s(opt.gps_tow, g, az, el) * C * (L1_HZ / o.carrier).powi(2);
                    }
                }
                if opt.tropo {
                    corr += saastamoinen(g[0], g[2], el);
                }
            }
            let gi = groups
                .iter()
                .position(|&x| x == o.group)
                .expect("the group list holds the group of every used observation");
            let res = o.pr - (r + clk[gi] + corr);
            let w = (el.max(3f64.to_radians()).sin()).powi(2) * o.weight;
            let mut row = vec![0.0; nu];
            row[0] = -e[0];
            row[1] = -e[1];
            row[2] = -e[2];
            row[3 + gi] = 1.0;
            for a in 0..nu {
                atb[a] += w * row[a] * res;
                for b in 0..nu {
                    ata[a][b] += w * row[a] * row[b];
                }
            }
            out_h.push([-e[0], -e[1], -e[2]]);
            out_r.push(res);
            out_w.push(w);
            out_ae.push((az, el));
        }
        let inv = invert(&ata)?;
        let mut dx = vec![0.0; nu];
        for a in 0..nu {
            dx[a] = (0..nu).map(|b| inv[a][b] * atb[b]).sum();
        }
        for k in 0..3 {
            pos[k] += dx[k];
        }
        for k in 0..ng {
            clk[k] += dx[3 + k];
        }
        let step = (dx[0].powi(2) + dx[1].powi(2) + dx[2].powi(2)).sqrt();
        if step < 1e-5 && it >= 2 {
            break;
        }
    }
    // Final residuals at the solution.
    let have_pos = true;
    let g = ecef_to_geodetic(pos);
    out_r.clear();
    for (n, &i) in use_idx.iter().enumerate() {
        let o = &obs[i];
        let tau = crate::geo::dist(o.pos, pos) / C;
        let sp = rotate(o.pos, tau);
        let r = crate::geo::dist(sp, pos);
        let (az, el) = az_el(g, pos, sp);
        let mut corr = 0.0;
        if have_pos && el > 0.5f64.to_radians() {
            if opt.iono {
                if let Some(k) = &opt.klobuchar {
                    corr += k.delay_s(opt.gps_tow, g, az, el) * C * (L1_HZ / o.carrier).powi(2);
                }
            }
            if opt.tropo {
                corr += saastamoinen(g[0], g[2], el);
            }
        }
        let gi = groups
            .iter()
            .position(|&x| x == o.group)
            .expect("the group list holds the group of every used observation");
        out_r.push(o.pr - (r + clk[gi] + corr));
        out_ae[n] = (az, el);
    }
    Some(Lsq {
        pos,
        clocks: clk,
        groups,
        resid: out_r,
        w: out_w,
        h: out_h,
        azel: out_ae,
        iterations,
    })
}

fn test_stat(l: &Lsq) -> f64 {
    l.resid
        .iter()
        .zip(&l.w)
        .map(|(r, w)| w * r * r)
        .sum::<f64>()
        / 25.0
}

/// Solves. `ngroups`: the number of clock groups (for the output's clock list).
pub fn solve(obs: &[Obs], ngroups: usize, opt: &SolveOptions) -> Option<Solution> {
    // A group with one satellite is dropped.
    let mut count = vec![0usize; ngroups.max(1)];
    for o in obs {
        if o.group < count.len() {
            count[o.group] += 1;
        }
    }
    let mut use_idx: Vec<usize> = (0..obs.len())
        .filter(|&i| count[obs[i].group] >= 2)
        .collect();
    let start = opt.prior.unwrap_or([0.0; 3]);
    let mut l = lsq(obs, &use_idx, start, opt)?;
    // Elevation mask.
    let mask = opt.mask_deg.to_radians();
    let keep: Vec<usize> = use_idx
        .iter()
        .zip(&l.azel)
        .filter(|(_, ae)| ae.1 >= mask)
        .map(|(&i, _)| i)
        .collect();
    if keep.len() < use_idx.len() {
        let mut cnt = vec![0usize; count.len()];
        for &i in &keep {
            cnt[obs[i].group] += 1;
        }
        let keep: Vec<usize> = keep
            .into_iter()
            .filter(|&i| cnt[obs[i].group] >= 2)
            .collect();
        if let Some(l2) = lsq(obs, &keep, l.pos, opt) {
            l = l2;
            use_idx = keep;
        }
    }
    // Residual check.
    let mut excluded = 0;
    if opt.raim {
        loop {
            let dof = use_idx.len() as i64 - (3 + l.groups.len()) as i64;
            if dof <= 0 {
                break;
            }
            let t = test_stat(&l);
            let lim = chi2_limit(dof as usize);
            if t <= lim {
                break;
            }
            let mut best: Option<(f64, Lsq, Vec<usize>)> = None;
            for k in 0..use_idx.len() {
                let trial: Vec<usize> = use_idx
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != k)
                    .map(|(_, &i)| i)
                    .collect();
                let mut cnt = vec![0usize; count.len()];
                for &i in &trial {
                    cnt[obs[i].group] += 1;
                }
                if trial.iter().any(|&i| cnt[obs[i].group] < 2) {
                    continue;
                }
                if let Some(lt) = lsq(obs, &trial, l.pos, opt) {
                    let d2 = trial.len() as i64 - (3 + lt.groups.len()) as i64;
                    let norm = if d2 > 0 {
                        test_stat(&lt) / chi2_limit(d2 as usize)
                    } else {
                        // No redundancy left: residuals are zero; the last way out.
                        1.0
                    };
                    if best.as_ref().is_none_or(|b| norm < b.0) {
                        best = Some((norm, lt, trial));
                    }
                }
            }
            match best {
                Some((norm, lt, trial)) if norm < t / lim => {
                    l = lt;
                    use_idx = trial;
                    excluded += 1;
                }
                _ => break,
            }
        }
    }
    let n = use_idx.len();
    let nu = 3 + l.groups.len();
    let geo = ecef_to_geodetic(l.pos);
    // DOP (unweighted, ENU) and the weighted covariance.
    let enu_h: Vec<[f64; 3]> = l.h.iter().map(|h| ecef_to_enu(geo, *h)).collect();
    let build = |weighted: bool| -> Option<Vec<Vec<f64>>> {
        let mut m = vec![vec![0.0; nu]; nu];
        for (k, (&i, h)) in use_idx.iter().zip(&enu_h).enumerate() {
            let gi = l
                .groups
                .iter()
                .position(|&x| x == obs[i].group)
                .expect("the group list holds the group of every used observation");
            let mut row = vec![0.0; nu];
            row[..3].copy_from_slice(h);
            row[3 + gi] = 1.0;
            let w = if weighted { l.w[k] } else { 1.0 };
            for a in 0..nu {
                for b in 0..nu {
                    m[a][b] += w * row[a] * row[b];
                }
            }
        }
        invert(&m)
    };
    let q = build(false)?;
    let qw = build(true)?;
    let dof = n as i64 - nu as i64;
    let wss: f64 = l.resid.iter().zip(&l.w).map(|(r, w)| w * r * r).sum();
    let sigma = if dof > 0 {
        (wss / dof as f64).sqrt().max(1.5)
    } else {
        5.0
    };
    let rms = (l.resid.iter().map(|r| r * r).sum::<f64>() / n as f64).sqrt();
    if !(-1000.0..=30_000.0).contains(&geo[2]) || rms > 200.0 {
        return None;
    }
    let mut clocks = vec![None; ngroups.max(1)];
    for (k, &g) in l.groups.iter().enumerate() {
        clocks[g] = Some(l.clocks[k]);
    }
    Some(Solution {
        pos: l.pos,
        geo,
        clocks,
        hdop: (q[0][0] + q[1][1]).sqrt(),
        vdop: q[2][2].sqrt(),
        pdop: (q[0][0] + q[1][1] + q[2][2]).sqrt(),
        tdop: (3..nu).map(|k| q[k][k]).sum::<f64>().sqrt(),
        herr: (qw[0][0] + qw[1][1]).max(0.0).sqrt() * sigma,
        verr: qw[2][2].max(0.0).sqrt() * sigma,
        rms,
        used: use_idx
            .iter()
            .enumerate()
            .map(|(k, &i)| (obs[i].sat, l.resid[k], l.azel[k].0, l.azel[k].1))
            .collect(),
        excluded,
        iterations: l.iterations,
    })
}

/// The receiver velocity (ECEF, m/s) and clock drift (m/s) from the Dopplers of the satellites used.
pub fn velocity(obs: &[Obs], sol: &Solution) -> Option<([f64; 3], f64)> {
    let used: Vec<&Obs> = obs
        .iter()
        .filter(|o| sol.used.iter().any(|u| u.0 == o.sat))
        .collect();
    if used.len() < 4 {
        return None;
    }
    let mut ata = vec![vec![0.0; 4]; 4];
    let mut atb = [0.0; 4];
    for o in &used {
        let tau = crate::geo::dist(o.pos, sol.pos) / C;
        let sp = rotate(o.pos, tau);
        let sv = rotate(o.vel, tau);
        let r = crate::geo::dist(sp, sol.pos);
        let e = [
            (sp[0] - sol.pos[0]) / r,
            (sp[1] - sol.pos[1]) / r,
            (sp[2] - sol.pos[2]) / r,
        ];
        let rr = -o.doppler * C / o.carrier;
        // rr = e·(v_sat − v_rx) + drift_rx − c·drift_sat
        let y = rr - (e[0] * sv[0] + e[1] * sv[1] + e[2] * sv[2]) + C * o.clock_drift;
        let row = [-e[0], -e[1], -e[2], 1.0];
        let (_, el) = az_el(sol.geo, sol.pos, sp);
        let w = el.max(3f64.to_radians()).sin().powi(2) * o.weight;
        for a in 0..4 {
            atb[a] += w * row[a] * y;
            for b in 0..4 {
                ata[a][b] += w * row[a] * row[b];
            }
        }
    }
    let inv = invert(&ata)?;
    let x: Vec<f64> = (0..4)
        .map(|a| (0..4).map(|b| inv[a][b] * atb[b]).sum())
        .collect();
    Some(([x[0], x[1], x[2]], x[3]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::geodetic_to_ecef;
    use crate::system::System;

    fn sky(rx: [f64; 3], g: [f64; 3]) -> Vec<[f64; 3]> {
        let mut v = Vec::new();
        for (az, el) in [
            (0.0, 80.0),
            (40.0, 30.0),
            (100.0, 50.0),
            (160.0, 20.0),
            (220.0, 60.0),
            (280.0, 15.0),
            (320.0, 40.0),
            (190.0, 70.0),
        ] {
            let (az, el) = (f64::to_radians(az), f64::to_radians(el));
            let enu = [el.cos() * az.sin(), el.cos() * az.cos(), el.sin()];
            let (sl, cl) = g[0].sin_cos();
            let (so, co) = g[1].sin_cos();
            let d = [
                -so * enu[0] - sl * co * enu[1] + cl * co * enu[2],
                co * enu[0] - sl * so * enu[1] + cl * so * enu[2],
                cl * enu[1] + sl * enu[2],
            ];
            let r = 21_000e3;
            v.push([rx[0] + r * d[0], rx[1] + r * d[1], rx[2] + r * d[2]]);
        }
        v
    }

    fn obs_for(rx: [f64; 3], sats: &[[f64; 3]], bias: f64) -> Vec<Obs> {
        sats.iter()
            .enumerate()
            .map(|(i, &p)| {
                // The satellite position at transmit in its own frame: un-rotate so the solver's rotation returns p.
                let tau = crate::geo::dist(p, rx) / C;
                let p0 = rotate(p, -tau);
                let r = crate::geo::dist(rotate(p0, crate::geo::dist(p0, rx) / C), rx);
                Obs {
                    sat: SatId::new(System::Gps, i as u16 + 1),
                    group: 0,
                    pos: p0,
                    vel: [0.0; 3],
                    pr: r + bias,
                    weight: 1.0,
                    doppler: 0.0,
                    carrier: L1_HZ,
                    clock_drift: 0.0,
                }
            })
            .collect()
    }

    fn opts() -> SolveOptions {
        SolveOptions {
            iono: false,
            tropo: false,
            ..Default::default()
        }
    }

    #[test]
    fn noise_free_bad_satellite() {
        let g = [0.44, 0.97, 10.0];
        let rx = geodetic_to_ecef(g);
        let sats = sky(rx, g);
        let obs = obs_for(rx, &sats, 1234.5);
        let s = solve(&obs, 1, &opts()).unwrap();
        assert!(crate::geo::dist(s.pos, rx) < 2e-3, "{:?}", s.pos);
        assert!((s.clocks[0].unwrap() - 1234.5).abs() < 2e-3);
        assert!(s.hdop > 0.0 && s.pdop >= s.hdop);
        // one satellite 100 m off is excluded.
        let mut bad = obs.clone();
        bad[3].pr += 100.0;
        let s = solve(&bad, 1, &opts()).unwrap();
        assert_eq!(s.excluded, 1);
        assert!(crate::geo::dist(s.pos, rx) < 8.0);
        let mut o = opts();
        o.raim = false;
        let s2 = solve(&bad, 1, &o).unwrap();
        assert!(crate::geo::dist(s2.pos, rx) > 3.0 * crate::geo::dist(s.pos, rx));
        // three satellites, no fix.
        assert!(solve(&obs[..3], 1, &opts()).is_none());
        // a far prior position.
        let mut o = opts();
        o.prior = Some([1e7, -1e7, 1e7]);
        let s = solve(&obs, 1, &o).unwrap();
        assert!(crate::geo::dist(s.pos, rx) < 8.0);
    }

    #[test]
    fn two_systems() {
        let g = [0.44, 0.97, 10.0];
        let rx = geodetic_to_ecef(g);
        let sats = sky(rx, g);
        let mut obs = obs_for(rx, &sats, 100.0);
        for o in obs.iter_mut().skip(5) {
            o.group = 1;
            o.pr += 37.25;
            o.sat.system = System::Glonass;
        }
        let s = solve(&obs, 2, &opts()).unwrap();
        assert!((s.clocks[1].unwrap() - s.clocks[0].unwrap() - 37.25).abs() < 5e-3);
        assert!(crate::geo::dist(s.pos, rx) < 8.0);
        // A lone satellite of a second system is ignored.
        let mut obs2 = obs_for(rx, &sats, 100.0);
        obs2[7].group = 1;
        let s = solve(&obs2, 2, &opts()).unwrap();
        assert!(s.clocks[1].is_none());
        assert_eq!(s.used.len(), 7);
    }

    #[test]
    fn dop_matches_a_separate_inversion() {
        let g = [0.44, 0.97, 10.0];
        let rx = geodetic_to_ecef(g);
        let sats = sky(rx, g);
        let obs = obs_for(rx, &sats, 0.0);
        let s = solve(&obs, 1, &opts()).unwrap();
        let mut m = [[0.0; 4]; 4];
        for p in &sats {
            let (az, el) = az_el(g, rx, *p);
            let r = [-el.cos() * az.sin(), -el.cos() * az.cos(), -el.sin(), 1.0];
            for a in 0..4 {
                for b in 0..4 {
                    m[a][b] += r[a] * r[b];
                }
            }
        }
        let q = invert4(&m).unwrap();
        let pdop = (q[0][0] + q[1][1] + q[2][2]).sqrt();
        assert!((s.pdop - pdop).abs() / pdop < 0.02);
        assert!(chi2_limit(4) > 15.0 && chi2_limit(4) < 20.0);
    }
}
