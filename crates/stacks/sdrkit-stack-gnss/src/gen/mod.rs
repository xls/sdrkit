//! The test signal: a simulated sky as clean complex baseband at unit mean power. Noise, the radio's frequency and clock errors
//! and other impairments come from the standard `gen_*` settings.

pub mod msgs;
pub mod orbit;

use crate::codes::{self, SignalKind, NH20};
use crate::system::{SatId, System};
use msgs::*;
use orbit::*;
use sdrkit_algo_fec::conv::codes::K7_171_133;
use sdrkit_algo_fec::conv::Encoder;
use sdrkit_algo_sim::rng::Rng;
use sdrkit_core::{Complex32, Generator, SignalInfo};
use sdrkit_util_bits::line::NrzM;
use sdrkit_util_geo::time::gnss::{glonass_from_utc, GLONASS_MINUS_UTC_S};
use sdrkit_util_geo::time::LeapSeconds;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};

const C: f64 = 299_792_458.0;
const L1: f64 = 1_575_420_000.0;

/// What the generator makes.
#[derive(Clone, Debug)]
pub struct GenOptions {
    /// Systems, as a mask of [`System::bit`].
    pub systems: u8,
    pub centre_hz: f64,
    pub rate_hz: f64,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub height_m: f64,
    pub zenith_cn0: f64,
    /// Only the highest n satellites (per system); `None` for all above the mask.
    pub max_sats: Option<usize>,
    pub mask_deg: f64,
    pub cold: bool,
    pub atmosphere: bool,
    pub seed: u64,
    pub ggto: bool,
    pub bdt_gps: bool,
    /// Structured SBAS content (the `sbas` generator) instead of random data.
    pub sbas_structured: bool,
    /// SBAS PRNs.
    pub sbas_prns: Vec<u16>,
    pub week: i64,
}

impl Default for GenOptions {
    fn default() -> Self {
        GenOptions {
            systems: System::Gps.bit(),
            centre_hz: L1,
            rate_hz: 4e6,
            lat_deg: 25.2,
            lon_deg: 55.36,
            height_m: 10.0,
            zenith_cn0: 44.0,
            max_sats: None,
            mask_deg: 5.0,
            cold: false,
            atmosphere: true,
            seed: 1,
            ggto: true,
            bdt_gps: true,
            sbas_structured: false,
            sbas_prns: vec![123, 127, 128],
            week: 2400,
        }
    }
}

/// GPS time of week at the start: 354 480 + 22 s (warm) or + 13 s (cold).
pub fn start_tow(cold: bool) -> f64 {
    354_480.0 + if cold { 13.0 } else { 22.0 }
}

/// GST − GPS broadcast by the generator, s.
pub const GGTO_S: f64 = 25e-9;
/// BDT − GPS less 14 s, s.
pub const BDT_GPS_S: f64 = 30e-9;
/// GLONASS τGPS and τc, s.
pub const TAU_GPS: f64 = 20e-9;
pub const TAU_C: f64 = -12e-9;
/// Toe of the generator's ephemerides.
pub const TOE: f64 = 352_800.0;
/// Almanac toa.
pub const TOA: f64 = 356_352.0;

/// An orbit of the simulation.
#[derive(Clone, Debug)]
pub enum Orbit {
    Kepler(Elements, Consts),
    Fixed(V3),
    Glo(GloTruth),
}

/// How a satellite's clock runs: Kepler clock (toc, af0, af1, af2, group delay) or GLONASS (τn, γn, tb).
#[derive(Clone, Copy, Debug)]
pub enum Clock {
    Kepler(f64, f64, f64, f64, f64),
    Glo(f64, f64, f64),
    None,
}

/// The generator's data for one satellite (also its truth for tests).
#[derive(Clone, Debug)]
pub struct SimSat {
    pub id: SatId,
    pub glo_k: i8,
    pub slot: u16,
    pub kind: SignalKind,
    pub carrier: f64,
    pub orbit: Orbit,
    pub clock: Clock,
    pub raw: KeplerRaw,
    pub gal: GalExtra,
    pub glo_raw: [i64; 9],
    pub glo_tb: f64,
    pub cn0: f64,
    pub az: f64,
    pub el: f64,
    /// Whether the signal is rendered (inside the band and above the mask).
    pub rendered: bool,
    code: Vec<f32>,
    units_per_chip: f64,
    amp: f32,
}

/// The data of one satellite, cached by unit.
#[derive(Clone, Debug, Default)]
struct DataCache {
    unit: i64,
    bits: Vec<u8>,
    enc_state: Option<Encoder>,
    last_bits: Vec<u8>,
}

/// The generator.
pub struct GnssGenerator {
    pub opt: GenOptions,
    pub sats: Vec<SimSat>,
    pub rx: V3,
    t0: f64,
    n: u64,
    almanac: Vec<AlmRaw>,
    iono_utc: IonoUtcRaw,
    cache: Vec<DataCache>,
    data_seed: u64,
    sky_cn0: f64,
    /// GPS - UTC, s.
    leap_s: f64,
}

impl GnssGenerator {
    /// A generator with these options.
    pub fn new(opt: GenOptions) -> GnssGenerator {
        let lat = opt.lat_deg.to_radians();
        let lon = opt.lon_deg.to_radians();
        let rx = llh_to_xyz(lat, lon, opt.height_m);
        let t0 = start_tow(opt.cold);
        // GPS - UTC at the start, from the built-in leap-second table (GPS time is after 1980, inside the table).
        let leap_s = crate::geo::leap_at(opt.week, t0, &LeapSeconds::BUILTIN).unwrap_or(0);
        let mut g = GnssGenerator {
            rx,
            t0,
            n: 0,
            almanac: Vec::new(),
            iono_utc: IonoUtcRaw::typical(opt.week, leap_s as i64),
            leap_s: leap_s as f64,
            cache: Vec::new(),
            data_seed: opt.seed ^ 0x5555,
            sky_cn0: 0.0,
            sats: Vec::new(),
            opt,
        };
        g.iono_utc.alpha = [13, 2, -1, -1];
        g.iono_utc.beta = [44, 3, -2, -4];
        g.iono_utc.a0 = -2;
        g.iono_utc.a1 = 3;
        let mut s = Rng::new(g.opt.seed);
        g.build_sky(&mut s);
        g.cache = vec![DataCache::default(); g.sats.len()];
        g
    }

    /// The satellites rendered.
    pub fn rendered(&self) -> impl Iterator<Item = &SimSat> {
        self.sats.iter().filter(|s| s.rendered)
    }

    /// GPS time of the first sample.
    pub fn start_time(&self) -> f64 {
        self.t0
    }

    /// The receiver's true position.
    pub fn receiver(&self) -> V3 {
        self.rx
    }

    fn want(&self, s: System) -> bool {
        self.opt.systems & s.bit() != 0
    }

    fn add_kepler(
        &mut self,
        id: SatId,
        el: Elements,
        consts: Consts,
        scales: &Scales,
        clk: (f64, f64, f64, f64, f64),
        iod: i64,
    ) {
        let mut raw = KeplerRaw::quantise(&el, clk, scales);
        raw.iode = iod;
        raw.iodc = iod;
        raw.ura = 2;
        let elq = raw.elements(scales, el.geo);
        let clkq = raw.clock(scales);
        let kind = SignalKind::of(id.system);
        self.sats.push(SimSat {
            id,
            glo_k: 0,
            slot: 0,
            kind,
            carrier: id.system.carrier_hz(),
            orbit: Orbit::Kepler(elq, consts),
            clock: Clock::Kepler(clkq.0, clkq.1, clkq.2, clkq.3, clkq.4),
            raw,
            gal: GalExtra::default(),
            glo_raw: [0; 9],
            glo_tb: 0.0,
            cn0: 0.0,
            az: 0.0,
            el: 0.0,
            rendered: false,
            code: Vec::new(),
            units_per_chip: 1.0,
            amp: 0.0,
        });
    }

    /// Satellite position (ECEF of the instant) at system time `t`.
    fn sat_pos(sat: &SimSat, t: f64) -> V3 {
        match &sat.orbit {
            Orbit::Kepler(el, k) => {
                let mut dt = t - el.toe;
                if dt > 302_400.0 {
                    dt -= 604_800.0;
                } else if dt < -302_400.0 {
                    dt += 604_800.0;
                }
                el.ecef(dt, k).0
            }
            Orbit::Fixed(p) => *p,
            Orbit::Glo(g) => g.pos(t),
        }
    }

    fn look_at(&self, p: V3) -> (f64, f64, f64) {
        look(
            self.rx,
            self.opt.lat_deg.to_radians(),
            self.opt.lon_deg.to_radians(),
            p,
        )
    }

    fn visible(&self, sats: &[SimSat], t: f64) -> (usize, f64) {
        let mut rows = Vec::new();
        for s in sats {
            let p = Self::sat_pos(s, self.sys_time(s.id.system, t));
            let (el, az, _) = self.look_at(p);
            if el > self.opt.mask_deg.to_radians() {
                rows.push([-el.cos() * az.sin(), -el.cos() * az.cos(), -el.sin(), 1.0]);
            }
        }
        (rows.len(), pdop(&rows))
    }

    /// System time of GPS time `t` for orbit evaluation (seconds of week; GLONASS: seconds of the GLONASS day).
    fn sys_time(&self, s: System, t: f64) -> f64 {
        match s {
            System::Beidou => t - 14.0,
            System::Glonass => (t + GLONASS_MINUS_UTC_S - self.leap_s).rem_euclid(86_400.0),
            _ => t,
        }
    }

    fn build_sky(&mut self, rs: &mut Rng) {
        let t = self.t0;
        let week = self.opt.week;
        // GPS: 6 planes of 5.
        if self.want(System::Gps) || self.want(System::Qzss) || self.want(System::Sbas) || true {
            let mut base = Vec::new();
            let mut params = Vec::new();
            for p in 0..6 {
                for k in 0..5 {
                    let i0 = (55.0 + (rs.uniform() - 0.5) * 1.6).to_radians();
                    let a = 26_560e3 + (rs.uniform() - 0.5) * 12e3;
                    let e = 0.001 + rs.uniform() * 0.014;
                    let w = rs.uniform() * TAU - PI;
                    let harm = (rs.uniform() - 0.5) * 2e-6;
                    params.push((p, k, i0, a, e, w, harm));
                }
            }
            let mut best = (usize::MAX, 0.0, 0.0);
            for step in 0..36 {
                let ph = (step as f64 * 10.0).to_radians();
                base.clear();
                for &(p, k, i0, a, e, w, harm) in &params {
                    base.push(self.gps_like(p, k, i0, a, e, w, harm, ph));
                }
                let (n, pd) = self.visible(&base, t);
                let score = if (9..=10).contains(&n) && pd < 3.0 {
                    0
                } else {
                    1 + (n as i64 - 9).unsigned_abs() as usize
                };
                if score < best.0 || (score == best.0 && pd < best.1) {
                    best = (score, pd, ph);
                }
            }
            if self.want(System::Gps) {
                for (j, &(p, k, i0, a, e, w, harm)) in params.iter().enumerate() {
                    let mut s = self.gps_like(p, k, i0, a, e, w, harm, best.2);
                    s.id = SatId::new(System::Gps, j as u16 + 1);
                    let el = match s.orbit {
                        Orbit::Kepler(el, _) => el,
                        _ => unreachable!(),
                    };
                    let af0 = (rs.uniform() - 0.5) * 4e-4;
                    let af1 = (rs.uniform() - 0.5) * 2e-11;
                    let tgd = (rs.uniform() - 0.5) * 1e-8;
                    let iod = 60 + j as i64;
                    self.add_kepler(s.id, el, GPS_C, &GPS_S, (TOE, af0, af1, 0.0, tgd), iod);
                    // Almanac from the same orbit.
                    let pi = crate::system::GPS_PI;
                    let q = |x: f64, sc: f64| (x / sc).round() as i64;
                    let dt = TOA - TOE;
                    let a_ = el.sqrt_a * el.sqrt_a;
                    let n = (GPS_C.mu / a_.powi(3)).sqrt() + el.delta_n;
                    self.almanac.push(AlmRaw {
                        prn: j as i64 + 1,
                        e: q(el.e, 2f64.powi(-21)),
                        toa: (TOA / 4096.0) as i64,
                        di: q((el.i0 + el.idot * dt) / pi - 0.3, 2f64.powi(-19)),
                        omega_dot: q(el.omega_dot / pi, 2f64.powi(-38)),
                        health: 0,
                        sqrt_a: q(el.sqrt_a, 2f64.powi(-11)),
                        omega0: q(wrap_pi(el.omega0 + el.omega_dot * dt) / pi, 2f64.powi(-23)),
                        omega: q(wrap_pi(el.omega) / pi, 2f64.powi(-23)),
                        m0: q(wrap_pi(el.m0 + n * dt) / pi, 2f64.powi(-23)),
                        af0: q(af0, 2f64.powi(-20)),
                        af1: q(af1, 2f64.powi(-38)),
                    });
                }
            }
        }
        if self.want(System::Qzss) {
            // Three IGSO on one ground track; the node chosen so they are in view.
            let mut best = (f64::NEG_INFINITY, 0.0);
            for step in 0..36 {
                let o = (step as f64 * 10.0).to_radians();
                let sats: Vec<SimSat> = (0..3).map(|k| self.qzss(k, o)).collect();
                let m = sats
                    .iter()
                    .map(|s| self.look_at(Self::sat_pos(s, t)).0)
                    .fold(f64::INFINITY, f64::min);
                if m > best.0 {
                    best = (m, o);
                }
            }
            for k in 0..3 {
                let s = self.qzss(k, best.1);
                let el = match s.orbit {
                    Orbit::Kepler(el, _) => el,
                    _ => unreachable!(),
                };
                let af0 = (rs.uniform() - 0.5) * 2e-4;
                self.add_kepler(
                    SatId::new(System::Qzss, 193 + k as u16),
                    el,
                    GPS_C,
                    &GPS_S,
                    (TOE, af0, 0.0, 0.0, 0.0),
                    20 + k as i64,
                );
            }
        }
        if self.want(System::Sbas) {
            let lon0 = self.opt.lon_deg;
            for (k, &prn) in self.opt.sbas_prns.clone().iter().enumerate() {
                let lon = (lon0 + [-25.0, 5.0, 35.0][k % 3]).to_radians();
                let r = 42_164_169.0;
                self.sats.push(SimSat {
                    id: SatId::new(System::Sbas, prn),
                    glo_k: 0,
                    slot: 0,
                    kind: SignalKind::L1Ca,
                    carrier: L1,
                    orbit: Orbit::Fixed([r * lon.cos(), r * lon.sin(), 0.0]),
                    clock: Clock::None,
                    raw: KeplerRaw::default(),
                    gal: GalExtra::default(),
                    glo_raw: [0; 9],
                    glo_tb: 0.0,
                    cn0: 0.0,
                    az: 0.0,
                    el: 0.0,
                    rendered: false,
                    code: Vec::new(),
                    units_per_chip: 1.0,
                    amp: 0.0,
                });
            }
        }
        if self.want(System::Galileo) {
            let mut best = (usize::MAX, 0.0);
            for step in 0..36 {
                let ph = (step as f64 * 10.0).to_radians();
                let sats: Vec<SimSat> = (0..24)
                    .map(|j| self.walker(j, 29_600e3, 56.0, ph, System::Galileo))
                    .collect();
                let (n, _) = self.visible(&sats, t);
                let score = if (7..=8).contains(&n) {
                    0
                } else {
                    1 + (n as i64 - 7).unsigned_abs() as usize
                };
                if score < best.0 {
                    best = (score, ph);
                }
            }
            for j in 0..24 {
                let s = self.walker(j, 29_600e3, 56.0, best.1, System::Galileo);
                let el = match s.orbit {
                    Orbit::Kepler(el, _) => el,
                    _ => unreachable!(),
                };
                let af0 = (rs.uniform() - 0.5) * 2e-4;
                let af1 = (rs.uniform() - 0.5) * 1e-11;
                let bgd = (rs.uniform() - 0.5) * 8e-9;
                let id = SatId::new(System::Galileo, j as u16 + 1);
                self.add_kepler(id, el, GAL_C, &GAL_S, (TOE, af0, af1, 0.0, bgd), 50);
                let last = self
                    .sats
                    .last_mut()
                    .expect("add_kepler appends a satellite");
                last.gal = GalExtra {
                    iodnav: 50,
                    svid: j as i64 + 1,
                    bgd_a: last.raw.tgd,
                    bgd_b: last.raw.tgd,
                    ggto: self.opt.ggto.then_some((
                        (GGTO_S / 2f64.powi(-35)).round() as i64,
                        0,
                        ((t / 3600.0).floor() as i64) % 256,
                        week % 64,
                    )),
                };
            }
        }
        if self.want(System::Beidou) {
            // MEO Walker 24/3/1 on codes 9..14 and 19..36, IGSO on 6..8, GEO on 1 and 2.
            let meo_prns: Vec<u16> = (9..=14).chain(19..=36).collect();
            let mut best = (usize::MAX, 0.0);
            for step in 0..36 {
                let ph = (step as f64 * 10.0).to_radians();
                let sats: Vec<SimSat> = (0..24)
                    .map(|j| self.walker(j, 27_906e3, 55.0, ph, System::Beidou))
                    .collect();
                let (n, _) = self.visible(&sats, t);
                let score = if (6..=9).contains(&n) {
                    0
                } else {
                    1 + (n as i64 - 7).unsigned_abs() as usize
                };
                if score < best.0 {
                    best = (score, ph);
                }
            }
            for (j, &prn) in meo_prns.iter().enumerate() {
                let s = self.walker(j, 27_906e3, 55.0, best.1, System::Beidou);
                let el = match s.orbit {
                    Orbit::Kepler(el, _) => el,
                    _ => unreachable!(),
                };
                self.bds_add(prn, el, rs);
            }
            for k in 0..3 {
                let mut s = self.qzss(k, (self.opt.lon_deg - 20.0).to_radians() + 1.0);
                if let Orbit::Kepler(el, _) = &mut s.orbit {
                    el.i0 = 55f64.to_radians();
                    el.e = 0.002;
                    let el = *el;
                    self.bds_add(6 + k as u16, el, rs);
                }
            }
            for (prn, lon) in [(1u16, 80.0f64), (2, 58.75)] {
                // A GEO: circular, inclination ~0, its node set so the earth-fixed longitude is `lon`.
                let a = 42_164_200.0f64;
                let el = Elements {
                    sqrt_a: a.sqrt(),
                    e: 0.0002,
                    i0: 0.02,
                    omega0: 0.0,
                    omega: 0.0,
                    m0: 0.0,
                    toe: TOE,
                    geo: true,
                    ..Default::default()
                };
                // Rotate the in-orbit phase so the satellite is at `lon` at toe.
                let (p, _) = el.ecef(0.0, &BDS_C);
                let now = p[1].atan2(p[0]);
                let mut el2 = el;
                el2.m0 = wrap_pi(lon.to_radians() - now);
                self.bds_add(prn, el2, rs);
            }
        }
        if self.want(System::Glonass) {
            self.build_glonass(rs);
        }
        // Visibility, C/N0 and what is rendered.
        let mask = self.opt.mask_deg.to_radians();
        let (lo, hi) = (
            self.opt.centre_hz - self.opt.rate_hz / 2.0,
            self.opt.centre_hz + self.opt.rate_hz / 2.0,
        );
        for i in 0..self.sats.len() {
            let tsys = self.sys_time(self.sats[i].id.system, t);
            let p = Self::sat_pos(&self.sats[i], tsys);
            let (el, az, r) = self.look_at(p);
            let s = &mut self.sats[i];
            s.el = el;
            s.az = az;
            let orbit_h = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 6_371e3;
            let sysoff = match s.id.system {
                System::Galileo => -1.5,
                System::Sbas => -2.0,
                System::Beidou | System::Glonass => -1.0,
                _ => 0.0,
            };
            s.cn0 = self.opt.zenith_cn0
                - 20.0 * (r / orbit_h).log10()
                - 8.0 * (1.0 - el.sin().max(0.0))
                + sysoff;
            let half = s.kind.chip_rate() * if s.kind == SignalKind::E1b { 2.0 } else { 1.0 };
            let inside = s.carrier - half * 0.5 > lo && s.carrier + half * 0.5 < hi;
            s.rendered = el > mask && inside;
        }
        if let Some(n) = self.opt.max_sats {
            for sys in System::ALL {
                let mut v: Vec<(f64, usize)> = self
                    .sats
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.id.system == sys && s.rendered)
                    .map(|(i, s)| (s.el, i))
                    .collect();
                v.sort_by(|a, b| b.0.total_cmp(&a.0));
                for (_, i) in v.into_iter().skip(n) {
                    self.sats[i].rendered = false;
                }
            }
        }
        // Unit mean power: each satellite's share follows its C/N0. With noise at C/N0 `sky_cn0`, every satellite has its own
        // C/N0.
        let total: f64 = self
            .sats
            .iter()
            .filter(|s| s.rendered)
            .map(|s| 10f64.powf(s.cn0 / 10.0))
            .sum();
        self.sky_cn0 = if total > 0.0 {
            10.0 * total.log10()
        } else {
            0.0
        };
        for s in self.sats.iter_mut() {
            s.amp = if total > 0.0 {
                (10f64.powf(s.cn0 / 10.0) / total).sqrt() as f32
            } else {
                0.0
            };
            let chips = match s.id.system {
                System::Galileo => {
                    // The test generator draws its own E1-B-like codes; the receiver must be given the same table.
                    crate::codes::E1bTable::synthetic(GALILEO_TABLE_SEED, 36)
                        .code(s.id.prn)
                        .expect(
                            "the synthetic table has a code for every generated Galileo satellite",
                        )
                        .to_vec()
                }
                System::Beidou => codes::bds_b1i(s.id.prn).unwrap_or_default(),
                System::Glonass => codes::glonass_code(),
                _ => codes::l1_ca(s.id.prn).unwrap_or_default(),
            };
            if s.kind == SignalKind::E1b {
                s.code = chips
                    .iter()
                    .flat_map(|&c| {
                        let v = if c == 0 { 1.0 } else { -1.0 };
                        [v, -v]
                    })
                    .collect();
                s.units_per_chip = 2.0;
            } else {
                s.code = chips
                    .iter()
                    .map(|&c| if c == 0 { 1.0 } else { -1.0 })
                    .collect();
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn gps_like(
        &self,
        p: usize,
        k: usize,
        i0: f64,
        a: f64,
        e: f64,
        w: f64,
        harm: f64,
        ph: f64,
    ) -> SimSat {
        let n = (GPS_C.mu / a.powi(3)).sqrt();
        let u_now = (k as f64 * 72.0 + p as f64 * 12.0).to_radians() + ph;
        // M at toe so the argument of latitude at the start is u_now.
        let dt = self.t0 - TOE;
        let m0 = wrap_pi(u_now - w - n * dt);
        let node = (p as f64 * 60.0).to_radians() + 0.3;
        let el = Elements {
            sqrt_a: a.sqrt(),
            e,
            i0,
            omega0: wrap_pi(node),
            omega: w,
            m0,
            delta_n: 4.5e-9,
            omega_dot: -8.0e-9,
            idot: 1e-10,
            cuc: harm,
            cus: -harm,
            crc: 150.0,
            crs: 30.0,
            cic: 5e-8,
            cis: -5e-8,
            toe: TOE,
            geo: false,
        };
        SimSat {
            id: SatId::new(System::Gps, 0),
            glo_k: 0,
            slot: 0,
            kind: SignalKind::L1Ca,
            carrier: L1,
            orbit: Orbit::Kepler(el, GPS_C),
            clock: Clock::None,
            raw: KeplerRaw::default(),
            gal: GalExtra::default(),
            glo_raw: [0; 9],
            glo_tb: 0.0,
            cn0: 0.0,
            az: 0.0,
            el: 0.0,
            rendered: false,
            code: Vec::new(),
            units_per_chip: 1.0,
            amp: 0.0,
        }
    }

    fn qzss(&self, k: usize, o: f64) -> SimSat {
        let a: f64 = 42_164_000.0;
        let el = Elements {
            sqrt_a: a.sqrt(),
            e: 0.075,
            i0: 41f64.to_radians(),
            omega0: wrap_pi(o + (k as f64 * 120.0).to_radians()),
            omega: 270f64.to_radians() - PI * 2.0,
            m0: wrap_pi(-(k as f64 * 120.0).to_radians()),
            toe: TOE,
            ..Default::default()
        };
        let mut s = self.gps_like(0, 0, 0.0, a, 0.0, 0.0, 0.0, 0.0);
        s.orbit = Orbit::Kepler(el, GPS_C);
        s
    }

    fn walker(&self, j: usize, a: f64, inc: f64, ph: f64, sys: System) -> SimSat {
        let plane = j / 8;
        let k = j % 8;
        let c = if sys == System::Beidou { BDS_C } else { GAL_C };
        let n = (c.mu / a.powi(3)).sqrt();
        let u_now = (k as f64 * 45.0 + plane as f64 * 15.0).to_radians() + ph;
        let dt = self.t0 - TOE;
        let el = Elements {
            sqrt_a: a.sqrt(),
            e: 0.0003 + 0.0001 * (j % 5) as f64,
            i0: inc.to_radians(),
            omega0: wrap_pi((plane as f64 * 120.0).to_radians() + 0.7),
            omega: 0.5,
            m0: wrap_pi(u_now - 0.5 - n * dt),
            delta_n: 3e-9,
            omega_dot: -6e-9,
            crc: 120.0,
            crs: -20.0,
            cuc: 1e-6,
            cus: -1e-6,
            toe: TOE,
            ..Default::default()
        };
        let mut s = self.gps_like(0, 0, 0.0, a, 0.0, 0.0, 0.0, 0.0);
        s.orbit = Orbit::Kepler(el, c);
        s
    }

    fn bds_add(&mut self, prn: u16, el: Elements, rs: &mut Rng) {
        let af0 = (rs.uniform() - 0.5) * 2e-4;
        let af1 = (rs.uniform() - 0.5) * 1e-11;
        let tgd = (rs.uniform() - 0.5) * 8e-9;
        // BDT toe: the same instant less 14 s is not a multiple of 8, so toe stays at its own BDT value.
        self.add_kepler(
            SatId::new(System::Beidou, prn),
            el,
            BDS_C,
            &BDS_S,
            (TOE, af0, af1, 0.0, tgd),
            (prn % 30) as i64,
        );
    }

    fn build_glonass(&mut self, rs: &mut Rng) {
        let tg = self.sys_time(System::Glonass, self.t0);
        let tb = (tg / 900.0).round() * 900.0;
        let a = 25_510_000.0f64;
        let mu = 398_600.44e9;
        let w = 7.292115e-5;
        let mk = |j: usize, ph: f64| -> [f64; 6] {
            let plane = j / 8;
            let k = j % 8;
            let inc = 64.8f64.to_radians();
            let node = (plane as f64 * 120.0).to_radians() + 0.4;
            let u = (k as f64 * 45.0 + plane as f64 * 15.0).to_radians() + ph;
            let v = (mu / a).sqrt();
            let r_orb = [a * u.cos(), a * u.sin(), 0.0];
            let v_orb = [-v * u.sin(), v * u.cos(), 0.0];
            let rot = |x: V3| {
                let (si, ci) = inc.sin_cos();
                let (so, co) = node.sin_cos();
                let y = [x[0], x[1] * ci, x[1] * si];
                [y[0] * co - y[1] * so, y[0] * so + y[1] * co, y[2]]
            };
            let ri = rot(r_orb);
            let vi = rot(v_orb);
            // Inertial to earth-fixed at tb: rotate by -theta, subtract w x r.
            let th = w * tb;
            let (s, c) = th.sin_cos();
            let re = [c * ri[0] + s * ri[1], -s * ri[0] + c * ri[1], ri[2]];
            let ve0 = [c * vi[0] + s * vi[1], -s * vi[0] + c * vi[1], vi[2]];
            let ve = [ve0[0] + w * re[1], ve0[1] - w * re[0], ve0[2]];
            [re[0], re[1], re[2], ve[0], ve[1], ve[2]]
        };
        let lat = self.opt.lat_deg.to_radians();
        let lon = self.opt.lon_deg.to_radians();
        let mut best = (usize::MAX, 0.0);
        for step in 0..36 {
            let ph = (step as f64 * 10.0).to_radians();
            let n = (0..24)
                .filter(|&j| {
                    let s = mk(j, ph);
                    look(self.rx, lat, lon, [s[0], s[1], s[2]]).0 > self.opt.mask_deg.to_radians()
                })
                .count();
            let score = if (7..=9).contains(&n) {
                0
            } else {
                1 + (n as i64 - 8).unsigned_abs() as usize
            };
            if score < best.0 {
                best = (score, ph);
            }
        }
        // Channels: the satellites in view get distinct channels, the two highest −7 and +6.
        let states: Vec<[f64; 6]> = (0..24).map(|j| mk(j, best.1)).collect();
        let mut vis: Vec<(f64, usize)> = states
            .iter()
            .enumerate()
            .map(|(j, s)| (look(self.rx, lat, lon, [s[0], s[1], s[2]]).0, j))
            .collect();
        vis.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut chan = [0i8; 24];
        let mut free: Vec<i8> = (-6..=5).collect();
        for (rank, &(_, j)) in vis.iter().enumerate() {
            chan[j] = match rank {
                0 => -7,
                1 => 6,
                _ => {
                    if free.is_empty() {
                        free = (-6..=5).collect();
                    }
                    free.remove(0)
                }
            };
        }
        let t_from = tg - 60.0;
        let t_to = tg + 600.0;
        for j in 0..24 {
            let s = states[j];
            // Quantise as broadcast: position 2^-11 km, velocity 2^-20 km/s.
            let qp = |x: f64| (x / 1000.0 / 2f64.powi(-11)).round() as i64;
            let qv = |x: f64| (x / 1000.0 / 2f64.powi(-20)).round() as i64;
            let raw = [
                qp(s[0]),
                qp(s[1]),
                qp(s[2]),
                qv(s[3]),
                qv(s[4]),
                qv(s[5]),
                0,
                0,
                0,
            ];
            let st = [
                raw[0] as f64 * 2f64.powi(-11) * 1000.0,
                raw[1] as f64 * 2f64.powi(-11) * 1000.0,
                raw[2] as f64 * 2f64.powi(-11) * 1000.0,
                raw[3] as f64 * 2f64.powi(-20) * 1000.0,
                raw[4] as f64 * 2f64.powi(-20) * 1000.0,
                raw[5] as f64 * 2f64.powi(-20) * 1000.0,
            ];
            let truth = GloTruth::new(tb, st, [0.0; 3], t_from, t_to);
            let tau_n_raw = ((rs.uniform() - 0.5) * 2e-4 / 2f64.powi(-30)).round() as i64;
            let gamma_raw = ((rs.uniform() - 0.5) * 4e-12 / 2f64.powi(-40)).round() as i64;
            let k = chan[j];
            self.sats.push(SimSat {
                id: SatId::new(System::Glonass, j as u16 + 1),
                glo_k: k,
                slot: j as u16 + 1,
                kind: SignalKind::G1,
                carrier: crate::system::glo_carrier(k),
                orbit: Orbit::Glo(truth),
                clock: Clock::Glo(
                    tau_n_raw as f64 * 2f64.powi(-30),
                    gamma_raw as f64 * 2f64.powi(-40),
                    tb,
                ),
                raw: KeplerRaw {
                    af0: tau_n_raw,
                    af1: gamma_raw,
                    ..Default::default()
                },
                gal: GalExtra::default(),
                glo_raw: raw,
                glo_tb: tb,
                cn0: 0.0,
                az: 0.0,
                el: 0.0,
                rendered: false,
                code: Vec::new(),
                units_per_chip: 1.0,
                amp: 0.0,
            });
        }
    }

    /// The C/N0 of the whole sky, dB-Hz. If the standard noise is set to this C/N0 (`gen_noise = cn0`), every satellite has its
    /// own C/N0 of [`SimSat::cn0`].
    pub fn sky_cn0_dbhz(&self) -> f64 {
        self.sky_cn0
    }

    /// The true Doppler (Hz) of a satellite at GPS time `t`, relative to its nominal carrier.
    pub fn truth_doppler(&self, id: SatId, t: f64) -> Option<f64> {
        let i = self.sats.iter().position(|s| s.id == id)?;
        let s = &self.sats[i];
        let lo = self.opt.centre_hz;
        let off = s.carrier - lo;
        let h = 0.01;
        let (_, a) = self.timing(i, t - h);
        let (_, b) = self.timing(i, t + h);
        let f = off + s.carrier * (b - a) / (2.0 * h);
        // Per second of the sample clock; and relative to the signal's nominal carrier (the band mixes its offset away).
        let centre_off = s.carrier - self.opt.centre_hz;
        Some(f - centre_off)
    }

    /// The code time of a satellite (its own scale) at GPS time `t`.
    pub fn truth_code_time(&self, id: SatId, t: f64) -> Option<f64> {
        let i = self.sats.iter().position(|s| s.id == id)?;
        Some(self.timing(i, t).0)
    }

    /// GPS time of sample `n`.
    pub fn time_of_sample(&self, n: f64) -> f64 {
        self.t0 + n / self.opt.rate_hz
    }

    /// The broadcast ephemeris of a satellite as the receiver's type (for tests: "equal to what was sent").
    pub fn broadcast_kepler(&self, id: SatId) -> Option<crate::orbit::Kepler> {
        let s = self.sats.iter().find(|s| s.id == id)?;
        let (scales, geo) = match id.system {
            System::Galileo => (GAL_S, false),
            System::Beidou => (BDS_S, crate::system::bds_is_geo(id.prn)),
            System::Gps | System::Qzss => (GPS_S, false),
            _ => return None,
        };
        let el = s.raw.elements(&scales, geo);
        let (toc, af0, af1, af2, tgd) = s.raw.clock(&scales);
        Some(crate::orbit::Kepler {
            system: Some(id.system),
            prn: id.prn,
            week: match id.system {
                System::Beidou => self.opt.week - 1356,
                _ => self.opt.week,
            },
            toe: el.toe,
            toc,
            sqrt_a: el.sqrt_a,
            e: el.e,
            m0: el.m0,
            delta_n: el.delta_n,
            omega0: el.omega0,
            i0: el.i0,
            omega: el.omega,
            omega_dot: el.omega_dot,
            idot: el.idot,
            cuc: el.cuc,
            cus: el.cus,
            crc: el.crc,
            crs: el.crs,
            cic: el.cic,
            cis: el.cis,
            af0,
            af1,
            af2,
            tgd,
            iode: s.raw.iode as u32,
            iodc: s.raw.iodc as u32,
            health: 0,
            ura: s.raw.ura as u32,
            fit: 0,
            geo,
        })
    }

    // ------------------------------------------------------------ signal timing

    /// System-time offset from GPS time for code timing (s).
    fn sys_offset(&self, s: System) -> f64 {
        match s {
            System::Galileo => {
                if self.opt.ggto {
                    GGTO_S
                } else {
                    0.0
                }
            }
            System::Beidou => -14.0 + if self.opt.bdt_gps { BDT_GPS_S } else { 0.0 },
            _ => 0.0,
        }
    }

    /// For satellite `i` at true GPS receive time `t`: (code time in the satellite's own scale, carrier phase term in seconds
    /// (−carrier delay + clock), geometric range).
    fn timing(&self, i: usize, t: f64) -> (f64, f64) {
        let s = &self.sats[i];
        let sys = s.id.system;
        // Transmit time by iteration, in GPS time.
        let mut te = t - 0.075;
        let mut range = 0.0;
        let mut pos = [0.0; 3];
        for _ in 0..4 {
            let tsys = match sys {
                System::Glonass => {
                    (te + GLONASS_MINUS_UTC_S - self.leap_s - TAU_GPS).rem_euclid(86_400.0)
                }
                System::Beidou => te - 14.0,
                _ => te,
            };
            let p = Self::sat_pos(s, tsys);
            let th = 7.2921151467e-5 * (t - te);
            let (sn, cs) = th.sin_cos();
            pos = [cs * p[0] + sn * p[1], -sn * p[0] + cs * p[1], p[2]];
            range = ((pos[0] - self.rx[0]).powi(2)
                + (pos[1] - self.rx[1]).powi(2)
                + (pos[2] - self.rx[2]).powi(2))
            .sqrt();
            te = t - range / C;
        }
        let (iono, tropo) = if self.opt.atmosphere {
            self.atmosphere(pos, t, s.carrier)
        } else {
            (0.0, 0.0)
        };
        let d_code = (range + tropo + iono) / C;
        let d_carr = (range + tropo - iono) / C;
        let te_code = t - d_code;
        let te_carr = t - d_carr;
        // Satellite clock offset at transmit.
        let clk = |te: f64| -> f64 {
            match s.clock {
                Clock::Kepler(toc, af0, af1, af2, tgd) => {
                    let tsys = te + self.sys_offset(sys);
                    let mut dt = tsys - toc;
                    if dt > 302_400.0 {
                        dt -= 604_800.0;
                    } else if dt < -302_400.0 {
                        dt += 604_800.0;
                    }
                    let rel = match &s.orbit {
                        Orbit::Kepler(el, k) => {
                            let mut d2 = tsys - el.toe;
                            if d2 > 302_400.0 {
                                d2 -= 604_800.0;
                            }
                            el.ecef(d2, k).1
                        }
                        _ => 0.0,
                    };
                    af0 + af1 * dt + af2 * dt * dt + rel - tgd
                }
                Clock::Glo(tau_n, gamma_n, tb) => {
                    let tg =
                        (te + GLONASS_MINUS_UTC_S - self.leap_s - TAU_GPS).rem_euclid(86_400.0);
                    -tau_n + gamma_n * (tg - tb)
                }
                Clock::None => 0.0,
            }
        };
        let code_time = match sys {
            System::Glonass => {
                (te_code + GLONASS_MINUS_UTC_S - self.leap_s - TAU_GPS).rem_euclid(86_400.0)
                    + clk(te_code)
            }
            _ => te_code + self.sys_offset(sys) + clk(te_code),
        };
        let carr = -d_carr + clk(te_carr);
        (code_time, carr)
    }

    /// (ionospheric delay at the carrier, tropospheric delay), m.
    fn atmosphere(&self, sat: V3, t: f64, carrier: f64) -> (f64, f64) {
        let lat = self.opt.lat_deg.to_radians();
        let lon = self.opt.lon_deg.to_radians();
        let (el, _, _) = look(self.rx, lat, lon, sat);
        if el <= 0.0 {
            return (0.0, 0.0);
        }
        let re = 6_371e3;
        let hs = 350e3;
        let m = 1.0 / (1.0 - (re * el.cos() / (re + hs)).powi(2)).sqrt();
        let local_h = ((t - self.leap_s) / 3600.0 + self.opt.lon_deg / 15.0).rem_euclid(24.0);
        let vert = 2.0 + 4.0 * ((local_h - 14.0) / 24.0 * TAU).cos().max(-0.5);
        let iono = vert * m * (L1 / carrier).powi(2);
        let zen = 2.3 * (-self.opt.height_m / 7000.0).exp();
        let tropo = zen * 1.001 / (0.002001 + el.sin().powi(2)).sqrt();
        (iono, tropo)
    }

    // ------------------------------------------------------------ data

    /// The data sign (×1 or ×−1) of satellite `i` for the code period starting at satellite time `ts` (its own scale).
    fn data_sign(&mut self, i: usize, ts: f64) -> f32 {
        let sys = self.sats[i].id.system;
        let bit = match sys {
            System::Gps | System::Qzss => {
                let sf = (ts / 6.0).floor() as i64;
                let k = ((ts - sf as f64 * 6.0) / 0.02).floor() as usize;
                self.unit_bits(i, sf);
                self.cache[i].bits[k.min(299)]
            }
            System::Sbas => {
                let m = ts.floor() as i64;
                let k = ((ts - m as f64) / 0.002).floor() as usize;
                self.unit_bits(i, m);
                self.cache[i].bits[k.min(499)]
            }
            System::Galileo => {
                let p = ts.floor() as i64;
                let k = ((ts - p as f64) / 0.004).floor() as usize;
                self.unit_bits(i, p);
                self.cache[i].bits[k.min(249)]
            }
            System::Beidou => {
                let geo = crate::system::bds_is_geo(self.sats[i].id.prn);
                if geo {
                    let sf = (ts / 0.6).floor() as i64;
                    let k = ((ts - sf as f64 * 0.6) / 0.002).floor() as usize;
                    self.unit_bits(i, sf);
                    self.cache[i].bits[k.min(299)]
                } else {
                    let sf = (ts / 6.0).floor() as i64;
                    let k = ((ts - sf as f64 * 6.0) / 0.02).floor() as usize;
                    let nh = NH20[((ts / 0.001).floor() as i64).rem_euclid(20) as usize];
                    self.unit_bits(i, sf);
                    self.cache[i].bits[k.min(299)] ^ nh
                }
            }
            System::Glonass => {
                let st = (ts / 2.0).floor() as i64;
                let x = ts - st as f64 * 2.0;
                self.unit_bits(i, st);
                if x < 1.7 {
                    let j = (x / 0.02).floor() as usize;
                    let half = ((x - j as f64 * 0.02) >= 0.01) as u8;
                    self.cache[i].bits[j.min(84)] ^ half
                } else {
                    let j = ((x - 1.7) / 0.01).floor() as usize;
                    codes::GLO_TIME_MARK[j.min(29)]
                }
            }
        };
        if bit == 0 {
            1.0
        } else {
            -1.0
        }
    }

    fn unit_bits(&mut self, i: usize, unit: i64) {
        if self.cache[i].unit == unit && !self.cache[i].bits.is_empty() {
            return;
        }
        let sys = self.sats[i].id.system;
        let bits = match sys {
            System::Gps | System::Qzss => {
                let s = &self.sats[i];
                let frame = unit.div_euclid(5);
                let f0 = (self.t0 / 30.0).floor() as i64;
                let page = (frame - f0 + 16).rem_euclid(25) + 1;
                let pages = LnavPages {
                    almanac: &self.almanac,
                    iono_utc: self.iono_utc,
                    wna: self.opt.week % 256,
                    toa: (TOA / 4096.0) as i64,
                    qzss: sys == System::Qzss,
                };
                lnav_subframe(&s.raw, self.opt.week % 1024, 0, unit, page, &pages)
            }
            System::Sbas => self.sbas_symbols(i, unit),
            System::Galileo => {
                let s = &self.sats[i];
                let page_start = unit - unit.rem_euclid(2);
                let slot = (page_start / 2).rem_euclid(15);
                let ty = inav_schedule(slot);
                let wn = (self.opt.week - 1024).rem_euclid(4096);
                let w = inav_word(ty, &s.raw, &s.gal, wn, page_start.rem_euclid(604_800));
                let (even, odd) = inav_page(&w);
                if unit % 2 == 0 {
                    inav_part_symbols(&even)
                } else {
                    inav_part_symbols(&odd)
                }
            }
            System::Beidou => {
                let s = &self.sats[i];
                let geo = crate::system::bds_is_geo(s.id.prn);
                let week = (self.opt.week - 1356).rem_euclid(8192);
                let f = bds_fields(&s.raw, week);
                let info = if geo {
                    let frame = unit.div_euclid(5);
                    let id = unit.rem_euclid(5) + 1;
                    let sow = (frame * 3).rem_euclid(604_800);
                    let mut body = vec![0u8; 186];
                    if id == 1 {
                        let page = frame.rem_euclid(10) + 1;
                        let mut stream: Vec<u8> = f.concat();
                        stream.resize(560, 0);
                        let p0 = (page as usize - 1) * 56;
                        for k in 0..4 {
                            body[k] = ((page >> (3 - k)) & 1) as u8;
                        }
                        body[4..60].copy_from_slice(&stream[p0..p0 + 56]);
                    }
                    bds_info(id, sow, &body)
                } else {
                    let id = unit.rem_euclid(5) + 1;
                    let sow = (unit * 6).rem_euclid(604_800);
                    let body = match id {
                        1..=3 => f[id as usize - 1].clone(),
                        _ => {
                            let frame = unit.div_euclid(5);
                            let page = frame.rem_euclid(24) + 1;
                            let mut b = vec![0u8; 186];
                            for k in 0..7 {
                                b[1 + k] = ((page >> (6 - k)) & 1) as u8;
                            }
                            if id == 5 && page == 9 && self.opt.bdt_gps {
                                let a0 = (BDT_GPS_S / 1e-10).round() as i64;
                                let base = 10 + 30;
                                for k in 0..14 {
                                    b[base + k] = ((a0 >> (13 - k)) & 1) as u8;
                                }
                            }
                            b
                        }
                    };
                    bds_info(id, sow, &body)
                };
                crate::bds::encode_subframe(&info)
            }
            System::Glonass => self.glo_string(i, unit),
        };
        self.cache[i].unit = unit;
        self.cache[i].bits = bits;
    }

    fn sbas_symbols(&mut self, i: usize, m: i64) -> Vec<u8> {
        let prn = self.sats[i].id.prn;
        let structured = self.opt.sbas_structured;
        let stream = |m: i64| Rng::stream(self.data_seed, ((m as u64) << 16) ^ prn as u64);
        let mut rng = stream(m);
        let mut next = || rng.next_u64();
        let make = |m: i64, next: &mut dyn FnMut() -> u64| -> Vec<u8> {
            let ty = SBAS_ROTATION[m.rem_euclid(16) as usize];
            let period = m.div_euclid(16);
            let data = if structured {
                sbas_content(prn, ty, m, period, &mut || next())
            } else {
                (0..212).map(|_| (next() & 1) as u8).collect()
            };
            sbas_message((m.rem_euclid(3)) as usize, ty, &data)
        };
        let cache = &mut self.cache[i];
        let mut enc = match cache.enc_state.take() {
            Some(e) if cache.unit == m - 1 => e,
            _ => {
                // Rebuild the state from the previous message.
                let mut rng2 = stream(m - 1);
                let mut next2 = || rng2.next_u64();
                let prev = make(m - 1, &mut next2);
                let mut e = Encoder::new(K7_171_133);
                let mut sink = Vec::new();
                e.encode(&prev[244..], &mut sink);
                e
            }
        };
        let msg = make(m, &mut next);
        let mut sym = Vec::with_capacity(500);
        enc.encode(&msg, &mut sym);
        let cache = &mut self.cache[i];
        cache.enc_state = Some(enc);
        cache.last_bits = msg;
        sym
    }

    fn glo_string(&self, i: usize, st: i64) -> Vec<u8> {
        let s = &self.sats[i];
        let m = st.rem_euclid(15) + 1;
        let frame_start = (st - st.rem_euclid(15)) * 2;
        let tod = frame_start.rem_euclid(86_400);
        let mut b = [0u8; 85];
        glo_set_u(&mut b, 84, 81, m);
        let r = &s.glo_raw;
        match m {
            1 => {
                glo_set_u(&mut b, 76, 72, tod / 3600);
                glo_set_u(&mut b, 71, 66, (tod % 3600) / 60);
                glo_set_u(&mut b, 65, 65, ((tod % 60) >= 30) as i64);
                glo_set_sm(&mut b, 64, 41, r[3]);
                glo_set_sm(&mut b, 40, 36, r[6]);
                glo_set_sm(&mut b, 35, 9, r[0]);
            }
            2 => {
                glo_set_u(&mut b, 80, 78, 0);
                glo_set_u(&mut b, 76, 70, (s.glo_tb / 900.0).round() as i64);
                glo_set_sm(&mut b, 64, 41, r[4]);
                glo_set_sm(&mut b, 40, 36, r[7]);
                glo_set_sm(&mut b, 35, 9, r[1]);
            }
            3 => {
                glo_set_sm(&mut b, 79, 69, s.raw.af1);
                glo_set_sm(&mut b, 64, 41, r[5]);
                glo_set_sm(&mut b, 40, 36, r[8]);
                glo_set_sm(&mut b, 35, 9, r[2]);
            }
            4 => {
                glo_set_sm(&mut b, 80, 59, s.raw.af0);
                let (n4, nt) = glo_day(self.opt.week, self.t0, self.leap_s);
                let _ = n4;
                glo_set_u(&mut b, 26, 16, nt);
                glo_set_u(&mut b, 15, 11, s.slot as i64);
                glo_set_u(&mut b, 10, 9, 1);
            }
            5 => {
                let (n4, nt) = glo_day(self.opt.week, self.t0, self.leap_s);
                glo_set_u(&mut b, 80, 70, nt);
                glo_set_sm(&mut b, 69, 38, (TAU_C / 2f64.powi(-31)).round() as i64);
                glo_set_u(&mut b, 36, 32, n4);
                glo_set_sm(&mut b, 31, 10, (TAU_GPS / 2f64.powi(-30)).round() as i64);
            }
            _ => {}
        }
        glo_finish(&mut b);
        // Relative code (GLONASS ICD clause 4.4): the string is sent NRZ-M from a 0 level, so bit 85 goes out as 0.
        let mut rel = b.to_vec();
        rel[0] = 0;
        NrzM::new(0).encode(&mut rel);
        rel
    }

    // ------------------------------------------------------------ rendering

    /// Renders `out.len()` samples.
    pub fn render(&mut self, out: &mut [Complex32]) {
        const BLOCK: usize = 4096;
        let mut done = 0;
        while done < out.len() {
            let n = BLOCK.min(out.len() - done);
            let block = &mut out[done..done + n];
            for v in block.iter_mut() {
                *v = Complex32::new(0.0, 0.0);
            }
            let na = self.n;
            let nb = self.n + n as u64;
            let ta = self.t0 + na as f64 / self.opt.rate_hz;
            let tb = self.t0 + nb as f64 / self.opt.rate_hz;
            for i in 0..self.sats.len() {
                if !self.sats[i].rendered {
                    continue;
                }
                self.render_sat(i, block, ta, tb);
            }
            self.n = nb;
            done += n;
        }
    }

    fn render_sat(&mut self, i: usize, block: &mut [Complex32], ta: f64, tb: f64) {
        let n = block.len();
        let (ca, pa) = self.timing(i, ta);
        let (cb, pb) = self.timing(i, tb);
        // The code is taken out for the loop, so the data can be looked up (which needs `&mut self`).
        let code = std::mem::take(&mut self.sats[i].code);
        let s = &self.sats[i];
        let carrier = s.carrier;
        let lo = self.opt.centre_hz;
        // Phase (cycles): (carrier − LO)·(t − t0) + carrier·(−delay + clock).
        let off = carrier - lo;
        let pha = off * (ta - self.t0) + carrier * pa;
        let phb = off * (tb - self.t0) + carrier * pb;
        let dph = (phb - pha) / n as f64;
        let period = s.kind.period_s();
        let units = code.len() as f64;
        let urate = units / period; // units per second of satellite time
        let du = (cb - ca) * urate / n as f64;
        let pidx = (ca / period).floor();
        let mut u = (ca - pidx * period) * urate;
        let mut period_start = pidx * period;
        let amp = s.amp;
        let ulen = code.len();
        let (sp, cp) = (TAU * pha.rem_euclid(1.0)).sin_cos();
        let mut z = Complex32::new(cp as f32, sp as f32);
        let (sr, cr) = (TAU * dph).sin_cos();
        let rot = Complex32::new(cr as f32, sr as f32);
        let mut sign = self.data_sign(i, period_start + 1e-7) * amp;
        for v in block.iter_mut() {
            // Boxcar of the code over [u, u + du).
            let u1 = u + du;
            let k = u.floor();
            let ki = k as usize;
            let val = if u1 <= k + 1.0 {
                code[ki % ulen]
            } else {
                let mut acc = (k + 1.0 - u) * code[ki % ulen] as f64;
                let mut c = k + 1.0;
                let mut j = ki + 1;
                while c + 1.0 <= u1 {
                    acc += code[j % ulen] as f64;
                    c += 1.0;
                    j += 1;
                }
                acc += (u1 - c) * code[j % ulen] as f64;
                (acc / du) as f32
            };
            *v += z * (sign * val);
            z *= rot;
            u = u1;
            if u >= units {
                u -= units;
                period_start += period;
                sign = self.data_sign(i, period_start + 1e-7) * amp;
            }
        }
        self.sats[i].code = code;
    }
}

fn wrap_pi(x: f64) -> f64 {
    let mut y = x.rem_euclid(TAU);
    if y > PI {
        y -= TAU;
    }
    y
}

fn pdop(rows: &[[f64; 4]]) -> f64 {
    if rows.len() < 4 {
        return 99.0;
    }
    let mut m = [[0.0f64; 4]; 4];
    for r in rows {
        for a in 0..4 {
            for b in 0..4 {
                m[a][b] += r[a] * r[b];
            }
        }
    }
    match crate::pvt::invert4(&m) {
        Some(inv) => (inv[0][0] + inv[1][1] + inv[2][2]).sqrt(),
        None => 99.0,
    }
}

/// N4 and NT of the GLONASS day at GPS week `week`, time `t`, with `leap_s` leap seconds (Moscow date).
fn glo_day(week: i64, t: f64, leap_s: f64) -> (i64, i64) {
    let utc = crate::geo::gps_unix(week, t) - leap_s;
    // GPS time is after 1996, so the GLONASS time is in range.
    glonass_from_utc(utc).map_or((0, 0), |g| (g.n4 as i64, g.nt as i64))
}

/// The seed of the synthetic Galileo code table the generator uses (see [`crate::codes::E1bTable::synthetic`]).
pub const GALILEO_TABLE_SEED: u64 = 0xE1B;

impl Generator for GnssGenerator {
    fn rate_hz(&self) -> f64 {
        self.opt.rate_hz
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        self.render(out);
        out.len()
    }
    fn signal_info(&self) -> Option<SignalInfo> {
        // The C/A code main lobe; the data rate of GPS LNAV.
        Some(SignalInfo::new(2.046e6).with_bit_rate(50.0))
    }
}

/// Satellite truths for tests: name → (az°, el°, C/N0).
pub fn sky_table(g: &GnssGenerator) -> BTreeMap<String, (f64, f64, f64)> {
    g.rendered()
        .map(|s| (s.id.name(), (s.az.to_degrees(), s.el.to_degrees(), s.cn0)))
        .collect()
}
