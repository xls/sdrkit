//! The fix of a measurement epoch: pseudoranges on a common time scale, the solver, the receiver clock, GPS time, UTC, the
//! velocity and the radio's frequency error. The receiver core and the stand-alone position component use it.

use crate::core::{Fix, MeasEpoch, SatMeas, SystemFix};
use crate::geo;
use crate::navstore::NavStore;
use crate::pvt::{self, Obs, SolveOptions};
use crate::system::*;
use sdrkit_util_geo::time::LeapSeconds;
use std::collections::BTreeMap;

/// The median of `v` (sorted in place), or `None` when it is empty.
pub(crate) fn median(v: &mut [f64]) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_unstable_by(|a, b| a.total_cmp(b));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        0.5 * (v[n / 2 - 1] + v[n / 2])
    })
}

/// The state the fix computation keeps from epoch to epoch.
#[derive(Clone, Debug, Default)]
pub struct FixSolver {
    pub week_ref: i64,
    pub mask_deg: f64,
    pub prior: Option<[f64; 3]>,
    /// GPS time minus signal time, set at the first fix.
    pub clock_offset: Option<f64>,
    pub first_fix_s: Option<f64>,
    pub fix_count: u64,
    pub freq_error: Option<f64>,
    pub reset_marker: f64,
    /// Satellites of the last fix: residual, azimuth, elevation.
    pub last_used: BTreeMap<SatId, (f64, f64, f64)>,
    pub last_fix: Option<Fix>,
    /// The leap-second table of the station: GPS - UTC until the satellites broadcast it.
    pub leap_table: LeapSeconds,
}

impl FixSolver {
    pub fn new(week_ref: i64, mask_deg: f64) -> FixSolver {
        FixSolver {
            week_ref,
            mask_deg,
            ..Default::default()
        }
    }

    /// Forgets the solution state.
    pub fn reset(&mut self, now: f64) {
        *self = FixSolver {
            week_ref: self.week_ref,
            mask_deg: self.mask_deg,
            reset_marker: now,
            leap_table: self.leap_table,
            ..Default::default()
        };
    }

    /// Solves the fix of a measurement epoch at signal time `t`.
    pub fn solve(&mut self, nav: &NavStore, ep: &MeasEpoch, t: f64) -> Option<Fix> {
        struct Cand {
            sat: SatId,
            tx_gps: f64,
            group: usize,
            m: SatMeas,
        }
        let week = nav.gps_week().unwrap_or(self.week_ref);
        // GPS - UTC: broadcast (IS-GPS-200 clause 20.3.3.5.2.4), else the station's table at the middle of the week.
        let leap = nav
            .utc
            .map(|u| u.dtls)
            .or_else(|| geo::leap_at(week, 302_400.0, &self.leap_table).ok());
        let mut cands = Vec::new();
        for m in &ep.sats {
            if m.cn0 < 24.0 {
                continue;
            }
            let sat = m.sat;
            let healthy = nav.healthy(sat).unwrap_or(false);
            if !healthy {
                continue;
            }
            let (tx_gps, group) = match sat.system {
                System::Gps | System::Qzss => {
                    if !nav.eph.contains_key(&sat) {
                        continue;
                    }
                    (m.tx_time, 0)
                }
                System::Galileo => {
                    if !nav.eph.contains_key(&sat) {
                        continue;
                    }
                    match nav.ggto {
                        Some(g) => (m.tx_time - g.offset(week, m.tx_time), 0),
                        None => (m.tx_time, 1),
                    }
                }
                System::Beidou => {
                    if !nav.eph.contains_key(&sat) {
                        continue;
                    }
                    match nav.bdt_gps {
                        Some((a0, a1)) => (m.tx_time + BDT_GPS_S - (a0 + a1 * m.tx_time), 0),
                        None => (m.tx_time + BDT_GPS_S, 2),
                    }
                }
                System::Glonass => {
                    let Some(e) = nav.glo_eph.get(&sat) else {
                        continue;
                    };
                    let (n4, tau_gps) = nav.glo_time.map(|g| (g.n4, g.tau_gps)).unwrap_or((0, 0.0));
                    let Some(leap) = leap.map(f64::from) else {
                        continue;
                    };
                    let day = if let Some(d) = crate::glo::glo_day_since_gps_epoch(n4, e.nt) {
                        d
                    } else {
                        // From the known week: the Moscow day of the newest GPS time.
                        let g = self
                            .last_fix
                            .as_ref()
                            .map(|f| f.week as f64 * WEEK_S + f.tow)
                            .unwrap_or(week as f64 * WEEK_S + 302_400.0);
                        ((g - leap + 10_800.0) / 86_400.0).floor() as i64
                    };
                    let (w, s) = crate::glo::glo_to_gps(day, m.tx_time, leap, tau_gps);
                    let mut s = s;
                    // Keep it in the same week as the GPS time scale used.
                    if w > week {
                        s += WEEK_S * (w - week) as f64;
                    } else if w < week {
                        s -= WEEK_S * (week - w) as f64;
                    }
                    (s, 3)
                }
                System::Sbas => continue,
            };
            // Age check.
            let age_ok = match sat.system {
                System::Glonass => nav
                    .glo_eph
                    .get(&sat)
                    .is_some_and(|e| e.age(m.tx_time).abs() <= 1800.0),
                _ => nav
                    .eph
                    .get(&sat)
                    .is_some_and(|e| e.age(m.tx_time).abs() <= 4.0 * 3600.0),
            };
            if !age_ok {
                continue;
            }
            cands.push(Cand {
                sat,
                tx_gps,
                group,
                m: m.clone(),
            });
        }
        if cands.len() < 4 {
            return None;
        }
        let mean = cands.iter().map(|c| c.tx_gps).sum::<f64>() / cands.len() as f64;
        let t_ref = ((mean + 0.075) * 1e6).round() / 1e6;
        let mut obs = Vec::new();
        for c in &cands {
            let (pos, vel, clk, drift, tgd) = match c.sat.system {
                System::Glonass => {
                    let e = &nav.glo_eph[&c.sat];
                    let mut tt = c.m.tx_time;
                    let mut clk = 0.0;
                    for _ in 0..3 {
                        clk = e.clock(tt);
                        tt = c.m.tx_time - clk;
                    }
                    let (p, v) = e.integrate(tt, 30.0);
                    (p, v, clk, e.gamma_n, 0.0)
                }
                _ => {
                    let e = &nav.eph[&c.sat];
                    let mut tt = c.m.tx_time;
                    let mut clk = 0.0;
                    for _ in 0..3 {
                        clk = e.clock(tt) - e.tgd;
                        tt = c.m.tx_time - clk;
                    }
                    let st = e.state(tt);
                    (st.pos, st.vel, clk, st.drift, 0.0)
                }
            };
            let pr = C * (t_ref - c.tx_gps + clk - tgd);
            let w = 10f64.powf((c.m.cn0 - 45.0) / 10.0).clamp(0.05, 2.0);
            obs.push(Obs {
                sat: c.sat,
                group: c.group,
                pos,
                vel,
                pr,
                weight: w,
                doppler: c.m.doppler,
                carrier: c.m.carrier,
                clock_drift: drift,
            });
        }
        let opt = SolveOptions {
            mask_deg: self.mask_deg,
            klobuchar: nav.iono,
            gps_tow: t_ref.rem_euclid(WEEK_S),
            prior: self.prior,
            raim: true,
            iono: true,
            tropo: true,
        };
        let sol = pvt::solve(&obs, 4, &opt)?;
        self.prior = Some(sol.pos);
        // The clock unknown that stands for GPS time.
        let gps_clock = sol.clocks[0]
            .or(sol.clocks[1])
            .or(sol.clocks[2])
            .or(sol.clocks[3])
            .unwrap_or(0.0);
        let gps_time = t_ref - gps_clock / C;
        let rx_clock = t + *self.clock_offset.get_or_insert(gps_time - t);
        let clock_err_m = (rx_clock - gps_time) * C;
        let week_fix = nav.gps_week().unwrap_or(self.week_ref);
        let tow = gps_time.rem_euclid(WEEK_S);
        let leap_known = nav.utc.is_some();
        let leap_i = nav
            .utc
            .map(|u| u.dtls)
            .or_else(|| geo::leap_at(week_fix, tow, &self.leap_table).ok());
        let utc = leap_i.map(|l| geo::utc_unix(week_fix, tow, l));
        // Velocity and the radio's frequency error.
        let vel = pvt::velocity(&obs, &sol);
        let mut errs = Vec::new();
        for o in &obs {
            if !sol.used.iter().any(|u| u.0 == o.sat) {
                continue;
            }
            let tau = geo::dist(o.pos, sol.pos) / C;
            let th = OMEGA_E * tau;
            let (s, c) = th.sin_cos();
            let sp = [
                c * o.pos[0] + s * o.pos[1],
                -s * o.pos[0] + c * o.pos[1],
                o.pos[2],
            ];
            let sv = [
                c * o.vel[0] + s * o.vel[1],
                -s * o.vel[0] + c * o.vel[1],
                o.vel[2],
            ];
            let r = geo::dist(sp, sol.pos);
            let e = [
                (sp[0] - sol.pos[0]) / r,
                (sp[1] - sol.pos[1]) / r,
                (sp[2] - sol.pos[2]) / r,
            ];
            let rr = e[0] * sv[0] + e[1] * sv[1] + e[2] * sv[2] - C * o.clock_drift;
            let expected = -rr * o.carrier / C;
            errs.push((o.doppler - expected) * L1_HZ / o.carrier);
        }
        if errs.len() >= 3 {
            self.freq_error = median(&mut errs);
        }
        let mut systems = Vec::new();
        for sys in [
            System::Gps,
            System::Qzss,
            System::Galileo,
            System::Beidou,
            System::Glonass,
        ] {
            let n = sol.used.iter().filter(|u| u.0.system == sys).count();
            if n == 0 {
                continue;
            }
            let g = cands
                .iter()
                .find(|c| c.sat.system == sys)
                .map(|c| c.group)
                .unwrap_or(0);
            systems.push(SystemFix {
                system: Some(sys),
                in_fix: true,
                satellites: n,
                clock_m: sol.clocks[g].unwrap_or(0.0),
            });
        }
        let mut names: Vec<&str> = systems
            .iter()
            .filter_map(|s| s.system.map(|x| x.name()))
            .collect();
        names.dedup();
        let type_text = format!("{} {} satellites", names.join(" + "), sol.used.len());
        let (speed, course, venu, has_v, drift) = match vel {
            Some((v, d)) => {
                let enu = geo::ecef_to_enu(sol.geo, v);
                let sp = (enu[0] * enu[0] + enu[1] * enu[1]).sqrt();
                let course =
                    (sp >= 0.5).then(|| enu[0].atan2(enu[1]).to_degrees().rem_euclid(360.0));
                (sp, course, enu, true, d)
            }
            None => (0.0, None, [0.0; 3], false, 0.0),
        };
        if self.first_fix_s.is_none() {
            self.first_fix_s = Some(t - self.reset_marker);
        }
        self.fix_count += 1;
        let fix = Fix {
            valid: true,
            lat: sol.geo[0].to_degrees(),
            lon: sol.geo[1].to_degrees(),
            height: sol.geo[2],
            ecef: sol.pos,
            systems,
            galileo_offset_ns: nav
                .ggto
                .filter(|_| {
                    cands
                        .iter()
                        .any(|c| c.sat.system == System::Galileo && c.group == 0)
                })
                .map(|g| g.offset(week, tow) * 1e9),
            bdt_offset_ns: nav
                .bdt_gps
                .filter(|_| {
                    cands
                        .iter()
                        .any(|c| c.sat.system == System::Beidou && c.group == 0)
                })
                .map(|b| b.0 * 1e9),
            glo_tau_gps_ns: nav.glo_time.map(|g| g.tau_gps * 1e9),
            glo_tau_c_ns: nav.glo_time.map(|g| g.tau_c * 1e9),
            hdop: sol.hdop,
            vdop: sol.vdop,
            pdop: sol.pdop,
            tdop: sol.tdop,
            herr: sol.herr,
            verr: sol.verr,
            rms: sol.rms,
            type_text,
            first_fix_s: self.first_fix_s.unwrap_or(0.0),
            fix_count: self.fix_count,
            week: week_fix,
            tow,
            leap: leap_i,
            leap_known,
            utc,
            has_velocity: has_v,
            speed,
            course,
            vel_enu: venu,
            clock_bias_m: clock_err_m,
            clock_drift_ms: drift,
            signal_s: t,
            satellites: sol.used.len(),
            excluded: sol.excluded,
        };
        self.last_used.clear();
        for u in &sol.used {
            self.last_used.insert(u.0, (u.1, u.2, u.3));
        }
        self.last_fix = Some(fix.clone());
        Some(fix)
    }
}
