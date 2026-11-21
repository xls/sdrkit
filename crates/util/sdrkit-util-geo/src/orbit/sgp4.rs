//! The SGP4/SDP4 orbit propagator for NORAD element sets: SGP4 for near-Earth orbits (period under 225 minutes), SDP4 with the
//! lunar and solar perturbations and the resonance integration for deep-space orbits. The output is position and velocity in the
//! TEME frame in km and km/s; [`crate::orbit::teme_to_ecef`] converts it to Earth-fixed coordinates.
//!
//! Standard: Hoots and Roehrich, "Spacetrack Report #3" (1980); Vallado et al., "Revisiting Spacetrack Report #3", AIAA 2006-6753.

#![allow(clippy::excessive_precision, clippy::too_many_lines)]

use super::tle::Tle;
use crate::Error;
use std::f64::consts::{PI, TAU};

/// The gravity constants SGP4 runs with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gravity {
    /// WGS-72 as published in Spacetrack Report #3 (xke fixed at 0.0743669161).
    Wgs72Old,
    /// WGS-72 (μ = 398 600.8 km³/s², R = 6378.135 km). The constants NORAD element sets are fitted with; use this.
    #[default]
    Wgs72,
    /// WGS-84 (μ = 398 600.5 km³/s², R = 6378.137 km). Not the constants of the element sets; for comparison only.
    Wgs84,
}

/// Which of two small variants of the published code to follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OpsMode {
    /// The behaviour of the operational code of the US Air Force Space Command (keeps the node in 0..2π in the Lyddane
    /// modification). The published verification output uses this mode.
    Afspc,
    /// The "improved" mode of AIAA 2006-6753.
    #[default]
    Improved,
}

#[derive(Clone, Copy, Debug)]
struct Constants {
    radius_km: f64,
    xke: f64,
    j2: f64,
    j4: f64,
    j3oj2: f64,
}

impl Gravity {
    fn constants(self) -> Constants {
        let (mu, radius_km, xke_fixed, j2, j3, j4): (f64, f64, Option<f64>, f64, f64, f64) =
            match self {
                Gravity::Wgs72Old => (
                    398_600.799_64,
                    6378.135,
                    Some(0.074_366_916_1),
                    0.001_082_616,
                    -0.000_002_538_81,
                    -0.000_001_655_97,
                ),
                Gravity::Wgs72 => (
                    398_600.8,
                    6378.135,
                    None,
                    0.001_082_616,
                    -0.000_002_538_81,
                    -0.000_001_655_97,
                ),
                Gravity::Wgs84 => (
                    398_600.5,
                    6378.137,
                    None,
                    0.001_082_629_989_05,
                    -0.000_002_532_153_06,
                    -0.000_001_610_987_61,
                ),
            };
        let xke = xke_fixed.unwrap_or(60.0 / (radius_km * radius_km * radius_km / mu).sqrt());
        Constants {
            radius_km,
            xke,
            j2,
            j4,
            j3oj2: j3 / j2,
        }
    }
}

/// A position and velocity in the TEME frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TemeState {
    /// Position, km.
    pub r_km: [f64; 3],
    /// Velocity, km/s.
    pub v_km_s: [f64; 3],
}

/// Deep-space terms (SDP4).
#[derive(Clone, Copy, Debug, Default)]
struct Deep {
    irez: u8,
    d2201: f64,
    d2211: f64,
    d3210: f64,
    d3222: f64,
    d4410: f64,
    d4422: f64,
    d5220: f64,
    d5232: f64,
    d5421: f64,
    d5433: f64,
    dedt: f64,
    del1: f64,
    del2: f64,
    del3: f64,
    didt: f64,
    dmdt: f64,
    dnodt: f64,
    domdt: f64,
    e3: f64,
    ee2: f64,
    peo: f64,
    pgho: f64,
    pho: f64,
    pinco: f64,
    plo: f64,
    se2: f64,
    se3: f64,
    sgh2: f64,
    sgh3: f64,
    sgh4: f64,
    sh2: f64,
    sh3: f64,
    si2: f64,
    si3: f64,
    sl2: f64,
    sl3: f64,
    sl4: f64,
    gsto: f64,
    xfact: f64,
    xgh2: f64,
    xgh3: f64,
    xgh4: f64,
    xh2: f64,
    xh3: f64,
    xi2: f64,
    xi3: f64,
    xl2: f64,
    xl3: f64,
    xl4: f64,
    xlamo: f64,
    zmol: f64,
    zmos: f64,
}

/// SGP4 elements initialised for propagation.
#[derive(Clone, Debug)]
pub struct Sgp4 {
    c: Constants,
    ops: OpsMode,
    deep_space: bool,
    isimp: bool,
    epoch_days_1950: f64,
    epoch_unix: f64,
    // Mean elements.
    bstar: f64,
    ecco: f64,
    argpo: f64,
    inclo: f64,
    mo: f64,
    no_unkozai: f64,
    nodeo: f64,
    // Near-Earth terms.
    aycof: f64,
    con41: f64,
    cc1: f64,
    cc4: f64,
    cc5: f64,
    d2: f64,
    d3: f64,
    d4: f64,
    delmo: f64,
    eta: f64,
    argpdot: f64,
    omgcof: f64,
    sinmao: f64,
    t2cof: f64,
    t3cof: f64,
    t4cof: f64,
    t5cof: f64,
    x1mth2: f64,
    x7thm1: f64,
    mdot: f64,
    nodedot: f64,
    xlcof: f64,
    xmcof: f64,
    nodecf: f64,
    ds: Deep,
}

/// Mean elements in the units SGP4 initialisation takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    /// Epoch, days since 1949-12-31 00:00 UTC (Julian day − 2 433 281.5).
    pub epoch_days_1950: f64,
    /// B* drag term, inverse Earth radii.
    pub bstar: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Argument of perigee, radians.
    pub arg_perigee_rad: f64,
    /// Inclination, radians.
    pub inclination_rad: f64,
    /// Mean anomaly, radians.
    pub mean_anomaly_rad: f64,
    /// Mean motion (Kozai), radians per minute.
    pub mean_motion_rad_min: f64,
    /// Right ascension of the ascending node, radians.
    pub raan_rad: f64,
}

/// Days from 1949-12-31 00:00 (SGP4's day 0, Julian day 2 433 281.5) to 1970-01-01 00:00 (Julian day 2 440 587.5).
const DAYS_1950_TO_UNIX: f64 = 7_306.0;

impl Elements {
    /// The elements of a TLE.
    pub fn from_tle(t: &Tle) -> Elements {
        let xpdotp = 1440.0 / TAU;
        Elements {
            epoch_days_1950: epoch_days_1950(t),
            bstar: t.bstar,
            eccentricity: t.eccentricity,
            arg_perigee_rad: t.arg_perigee_deg.to_radians(),
            inclination_rad: t.inclination_deg.to_radians(),
            mean_anomaly_rad: t.mean_anomaly_deg.to_radians(),
            mean_motion_rad_min: t.mean_motion_rev_day / xpdotp,
            raan_rad: t.raan_deg.to_radians(),
        }
    }
}

/// The epoch in SGP4's days since 1949-12-31, formed as the published code forms it: the Julian day of the date plus the
/// fraction of the day, summed as one `f64` (which rounds it to about 40 µs), minus 2 433 281.5. Forming it this way reproduces
/// the published verification output exactly; the rounding is far below the accuracy of any element set.
fn epoch_days_1950(t: &Tle) -> f64 {
    let days = t.epoch_days_since_1970();
    let whole = days.floor();
    let jd = 2_440_587.5 + whole;
    (jd + (days - whole)) - 2_433_281.5
}

/// Greenwich mean sidereal time of SGP4 (IAU 1982, Vallado's `gstime`), radians 0..2π, of a UT1 Julian day.
pub fn gstime(jd_ut1: f64) -> f64 {
    let tut1 = (jd_ut1 - 2_451_545.0) / 36_525.0;
    let temp = -6.2e-6 * tut1 * tut1 * tut1
        + 0.093_104 * tut1 * tut1
        + (876_600.0 * 3600.0 + 8_640_184.812_866) * tut1
        + 67_310.548_41;
    (temp.to_radians() / 240.0).rem_euclid(TAU)
}

/// The C library's `fmod`: the remainder with the sign of `x`.
#[inline]
fn fmod(x: f64, y: f64) -> f64 {
    x % y
}

impl Sgp4 {
    /// Initialises SGP4 from a TLE with the WGS-72 constants and the improved mode.
    ///
    /// ```
    /// use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
    /// use sdrkit_util_geo::orbit::Sgp4;
    /// let t = Tle::from_lines(None,
    ///     "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753",
    ///     "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667", Checksum::Require).unwrap();
    /// let s = Sgp4::from_tle(&t).unwrap();
    /// let r = s.propagate(0.0).unwrap().r_km;
    /// // AIAA 2006-6753 verification output for satellite 00005 at epoch.
    /// assert!((r[0] - 7022.46529266).abs() < 1e-6 && (r[1] + 1400.08296755).abs() < 1e-6);
    /// ```
    pub fn from_tle(tle: &Tle) -> Result<Sgp4, Error> {
        Self::new(&Elements::from_tle(tle), Gravity::Wgs72, OpsMode::Improved)
    }

    /// Initialises SGP4 (the published `sgp4init`) from mean elements, gravity constants and mode.
    pub fn new(el: &Elements, gravity: Gravity, ops: OpsMode) -> Result<Sgp4, Error> {
        crate::error::finite(&[
            el.epoch_days_1950,
            el.bstar,
            el.eccentricity,
            el.arg_perigee_rad,
            el.inclination_rad,
            el.mean_anomaly_rad,
            el.mean_motion_rad_min,
            el.raan_rad,
        ])?;
        if !(0.0..1.0).contains(&el.eccentricity) {
            return Err(Error::Sgp4(1));
        }
        if el.mean_motion_rad_min <= 0.0 {
            return Err(Error::Sgp4(2));
        }
        let c = gravity.constants();
        let epoch = el.epoch_days_1950;
        let mut s = Sgp4 {
            c,
            ops,
            deep_space: false,
            isimp: false,
            epoch_days_1950: epoch,
            epoch_unix: (epoch - DAYS_1950_TO_UNIX) * 86_400.0,
            bstar: el.bstar,
            ecco: el.eccentricity,
            argpo: el.arg_perigee_rad,
            inclo: el.inclination_rad,
            mo: el.mean_anomaly_rad,
            no_unkozai: 0.0,
            nodeo: el.raan_rad,
            aycof: 0.0,
            con41: 0.0,
            cc1: 0.0,
            cc4: 0.0,
            cc5: 0.0,
            d2: 0.0,
            d3: 0.0,
            d4: 0.0,
            delmo: 0.0,
            eta: 0.0,
            argpdot: 0.0,
            omgcof: 0.0,
            sinmao: 0.0,
            t2cof: 0.0,
            t3cof: 0.0,
            t4cof: 0.0,
            t5cof: 0.0,
            x1mth2: 0.0,
            x7thm1: 0.0,
            mdot: 0.0,
            nodedot: 0.0,
            xlcof: 0.0,
            xmcof: 0.0,
            nodecf: 0.0,
            ds: Deep::default(),
        };
        let temp4 = 1.5e-12;
        let ss = 78.0 / c.radius_km + 1.0;
        let qzms2ttemp = (120.0 - 78.0) / c.radius_km;
        let qzms2t = qzms2ttemp * qzms2ttemp * qzms2ttemp * qzms2ttemp;
        let x2o3 = 2.0 / 3.0;

        // initl
        let eccsq = s.ecco * s.ecco;
        let omeosq = 1.0 - eccsq;
        let rteosq = omeosq.sqrt();
        let cosio = s.inclo.cos();
        let cosio2 = cosio * cosio;
        let ak = (c.xke / el.mean_motion_rad_min).powf(x2o3);
        let d1 = 0.75 * c.j2 * (3.0 * cosio2 - 1.0) / (rteosq * omeosq);
        let mut del = d1 / (ak * ak);
        let adel = ak * (1.0 - del * del - del * (1.0 / 3.0 + 134.0 * del * del / 81.0));
        del = d1 / (adel * adel);
        s.no_unkozai = el.mean_motion_rad_min / (1.0 + del);
        let ao = (c.xke / s.no_unkozai).powf(x2o3);
        let sinio = s.inclo.sin();
        let po = ao * omeosq;
        let con42 = 1.0 - 5.0 * cosio2;
        s.con41 = -con42 - cosio2 - cosio2;
        let posq = po * po;
        let rp = ao * (1.0 - s.ecco);
        s.ds.gsto = gstime(epoch + 2_433_281.5);

        // sgp4init
        s.isimp = rp < (220.0 / c.radius_km + 1.0);
        let mut sfour = ss;
        let mut qzms24 = qzms2t;
        let perige = (rp - 1.0) * c.radius_km;
        if perige < 156.0 {
            sfour = perige - 78.0;
            if perige < 98.0 {
                sfour = 20.0;
            }
            let qzms24temp = (120.0 - sfour) / c.radius_km;
            qzms24 = qzms24temp * qzms24temp * qzms24temp * qzms24temp;
            sfour = sfour / c.radius_km + 1.0;
        }
        let pinvsq = 1.0 / posq;
        let tsi = 1.0 / (ao - sfour);
        s.eta = ao * s.ecco * tsi;
        let etasq = s.eta * s.eta;
        let eeta = s.ecco * s.eta;
        let psisq = (1.0 - etasq).abs();
        let coef = qzms24 * tsi.powi(4);
        let coef1 = coef / psisq.powf(3.5);
        let cc2 = coef1
            * s.no_unkozai
            * (ao * (1.0 + 1.5 * etasq + eeta * (4.0 + etasq))
                + 0.375 * c.j2 * tsi / psisq * s.con41 * (8.0 + 3.0 * etasq * (8.0 + etasq)));
        s.cc1 = s.bstar * cc2;
        let cc3 = if s.ecco > 1.0e-4 {
            -2.0 * coef * tsi * c.j3oj2 * s.no_unkozai * sinio / s.ecco
        } else {
            0.0
        };
        s.x1mth2 = 1.0 - cosio2;
        s.cc4 = 2.0
            * s.no_unkozai
            * coef1
            * ao
            * omeosq
            * (s.eta * (2.0 + 0.5 * etasq) + s.ecco * (0.5 + 2.0 * etasq)
                - c.j2 * tsi / (ao * psisq)
                    * (-3.0 * s.con41 * (1.0 - 2.0 * eeta + etasq * (1.5 - 0.5 * eeta))
                        + 0.75
                            * s.x1mth2
                            * (2.0 * etasq - eeta * (1.0 + etasq))
                            * (2.0 * s.argpo).cos()));
        s.cc5 = 2.0 * coef1 * ao * omeosq * (1.0 + 2.75 * (etasq + eeta) + eeta * etasq);
        let cosio4 = cosio2 * cosio2;
        let temp1 = 1.5 * c.j2 * pinvsq * s.no_unkozai;
        let temp2 = 0.5 * temp1 * c.j2 * pinvsq;
        let temp3 = -0.46875 * c.j4 * pinvsq * pinvsq * s.no_unkozai;
        s.mdot = s.no_unkozai
            + 0.5 * temp1 * rteosq * s.con41
            + 0.0625 * temp2 * rteosq * (13.0 - 78.0 * cosio2 + 137.0 * cosio4);
        s.argpdot = -0.5 * temp1 * con42
            + 0.0625 * temp2 * (7.0 - 114.0 * cosio2 + 395.0 * cosio4)
            + temp3 * (3.0 - 36.0 * cosio2 + 49.0 * cosio4);
        let xhdot1 = -temp1 * cosio;
        s.nodedot = xhdot1
            + (0.5 * temp2 * (4.0 - 19.0 * cosio2) + 2.0 * temp3 * (3.0 - 7.0 * cosio2)) * cosio;
        let xpidot = s.argpdot + s.nodedot;
        s.omgcof = s.bstar * cc3 * s.argpo.cos();
        s.xmcof = if s.ecco > 1.0e-4 {
            -x2o3 * coef * s.bstar / eeta
        } else {
            0.0
        };
        s.nodecf = 3.5 * omeosq * xhdot1 * s.cc1;
        s.t2cof = 1.5 * s.cc1;
        s.xlcof = if (cosio + 1.0).abs() > 1.5e-12 {
            -0.25 * c.j3oj2 * sinio * (3.0 + 5.0 * cosio) / (1.0 + cosio)
        } else {
            -0.25 * c.j3oj2 * sinio * (3.0 + 5.0 * cosio) / temp4
        };
        s.aycof = -0.5 * c.j3oj2 * sinio;
        let delmotemp = 1.0 + s.eta * s.mo.cos();
        s.delmo = delmotemp * delmotemp * delmotemp;
        s.sinmao = s.mo.sin();
        s.x7thm1 = 7.0 * cosio2 - 1.0;

        if TAU / s.no_unkozai >= 225.0 {
            s.deep_space = true;
            s.isimp = true;
            let tc = 0.0;
            let inclm = s.inclo;
            let dc = dscom(
                epoch,
                s.ecco,
                s.argpo,
                tc,
                s.inclo,
                s.nodeo,
                s.no_unkozai,
                &mut s.ds,
            );
            // dpper at initialisation leaves the elements unchanged (init = 'y'), so it is not called here.
            let mut st = DsState {
                em: dc.em,
                argpm: 0.0,
                inclm,
                mm: 0.0,
                nm: dc.nm,
                nodem: 0.0,
            };
            dsinit(
                c.xke,
                &dc,
                s.argpo,
                0.0,
                tc,
                s.mo,
                s.mdot,
                s.no_unkozai,
                s.nodeo,
                s.nodedot,
                xpidot,
                s.ecco,
                eccsq,
                &mut st,
                &mut s.ds,
            );
        }

        if !s.isimp {
            let cc1sq = s.cc1 * s.cc1;
            s.d2 = 4.0 * ao * tsi * cc1sq;
            let temp = s.d2 * tsi * s.cc1 / 3.0;
            s.d3 = (17.0 * ao + sfour) * temp;
            s.d4 = 0.5 * temp * ao * tsi * (221.0 * ao + 31.0 * sfour) * s.cc1;
            s.t3cof = s.d2 + 2.0 * cc1sq;
            s.t4cof = 0.25 * (3.0 * s.d3 + s.cc1 * (12.0 * s.d2 + 10.0 * cc1sq));
            s.t5cof = 0.2
                * (3.0 * s.d4
                    + 12.0 * s.cc1 * s.d3
                    + 6.0 * s.d2 * s.d2
                    + 15.0 * cc1sq * (2.0 * s.d2 + cc1sq));
        }
        // The published initialisation propagates to the epoch and keeps any error.
        s.propagate(0.0)?;
        Ok(s)
    }

    /// The epoch as UTC Unix seconds.
    pub fn epoch_unix(&self) -> f64 {
        self.epoch_unix
    }

    /// Whether the orbit is deep space (period of 225 minutes or more), which adds the lunar-solar and resonance terms.
    pub fn is_deep_space(&self) -> bool {
        self.deep_space
    }

    /// The orbital period from the un-Kozai'd mean motion, minutes.
    pub fn period_min(&self) -> f64 {
        TAU / self.no_unkozai
    }

    /// The TEME state at Unix time `unix` (UTC).
    pub fn propagate_unix(&self, unix: f64) -> Result<TemeState, Error> {
        self.propagate((unix - self.epoch_unix) / 60.0)
    }

    /// The TEME state `tsince_min` minutes after the epoch (negative before it).
    pub fn propagate(&self, tsince_min: f64) -> Result<TemeState, Error> {
        if !tsince_min.is_finite() {
            return Err(Error::NotFinite);
        }
        let c = &self.c;
        let x2o3 = 2.0 / 3.0;
        let temp4 = 1.5e-12;
        let vkmpersec = c.radius_km * c.xke / 60.0;
        let t = tsince_min;

        let xmdf = self.mo + self.mdot * t;
        let argpdf = self.argpo + self.argpdot * t;
        let nodedf = self.nodeo + self.nodedot * t;
        let mut argpm = argpdf;
        let mut mm = xmdf;
        let t2 = t * t;
        let mut nodem = nodedf + self.nodecf * t2;
        let mut tempa = 1.0 - self.cc1 * t;
        let mut tempe = self.bstar * self.cc4 * t;
        let mut templ = self.t2cof * t2;

        if !self.isimp {
            let delomg = self.omgcof * t;
            let delmtemp = 1.0 + self.eta * xmdf.cos();
            let delm = self.xmcof * (delmtemp * delmtemp * delmtemp - self.delmo);
            let temp = delomg + delm;
            mm = xmdf + temp;
            argpm = argpdf - temp;
            let t3 = t2 * t;
            let t4 = t3 * t;
            tempa = tempa - self.d2 * t2 - self.d3 * t3 - self.d4 * t4;
            tempe += self.bstar * self.cc5 * (mm.sin() - self.sinmao);
            templ = templ + self.t3cof * t3 + t4 * (self.t4cof + t * self.t5cof);
        }

        let mut nm = self.no_unkozai;
        let mut em = self.ecco;
        let mut inclm = self.inclo;
        if self.deep_space {
            let mut st = DsState {
                em,
                argpm,
                inclm,
                mm,
                nm,
                nodem,
            };
            dspace(
                &self.ds,
                self.argpo,
                self.argpdot,
                t,
                self.no_unkozai,
                &mut st,
            );
            (em, argpm, inclm, mm, nm, nodem) = (st.em, st.argpm, st.inclm, st.mm, st.nm, st.nodem);
        }

        if nm <= 0.0 {
            return Err(Error::Sgp4(2));
        }
        let am = (c.xke / nm).powf(x2o3) * tempa * tempa;
        nm = c.xke / am.powf(1.5);
        em -= tempe;
        if !(-0.001..1.0).contains(&em) {
            return Err(Error::Sgp4(1));
        }
        if em < 1.0e-6 {
            em = 1.0e-6;
        }
        mm += self.no_unkozai * templ;
        let mut xlm = mm + argpm + nodem;
        nodem = fmod(nodem, TAU);
        argpm = fmod(argpm, TAU);
        xlm = fmod(xlm, TAU);
        mm = fmod(xlm - argpm - nodem, TAU);

        let sinim = inclm.sin();
        let cosim = inclm.cos();
        let mut ep = em;
        let mut xincp = inclm;
        let mut argpp = argpm;
        let mut nodep = nodem;
        let mut mp = mm;
        let mut sinip = sinim;
        let mut cosip = cosim;
        let (mut aycof, mut xlcof) = (self.aycof, self.xlcof);
        let (mut con41, mut x1mth2, mut x7thm1) = (self.con41, self.x1mth2, self.x7thm1);
        if self.deep_space {
            dpper(
                &self.ds, t, self.ops, &mut ep, &mut xincp, &mut nodep, &mut argpp, &mut mp,
            );
            if xincp < 0.0 {
                xincp = -xincp;
                nodep += PI;
                argpp -= PI;
            }
            if !(0.0..=1.0).contains(&ep) {
                return Err(Error::Sgp4(3));
            }
            sinip = xincp.sin();
            cosip = xincp.cos();
            aycof = -0.5 * c.j3oj2 * sinip;
            xlcof = if (cosip + 1.0).abs() > 1.5e-12 {
                -0.25 * c.j3oj2 * sinip * (3.0 + 5.0 * cosip) / (1.0 + cosip)
            } else {
                -0.25 * c.j3oj2 * sinip * (3.0 + 5.0 * cosip) / temp4
            };
        }

        // Long-period periodics.
        let axnl = ep * argpp.cos();
        let mut temp = 1.0 / (am * (1.0 - ep * ep));
        let aynl = ep * argpp.sin() + temp * aycof;
        let xl = mp + argpp + nodep + temp * xlcof * axnl;

        // Kepler's equation.
        let u = fmod(xl - nodep, TAU);
        let mut eo1 = u;
        let mut tem5: f64 = 9999.9;
        let mut ktr = 1;
        let (mut sineo1, mut coseo1) = (0.0, 0.0);
        while tem5.abs() >= 1.0e-12 && ktr <= 10 {
            sineo1 = eo1.sin();
            coseo1 = eo1.cos();
            tem5 = 1.0 - coseo1 * axnl - sineo1 * aynl;
            tem5 = (u - aynl * coseo1 + axnl * sineo1 - eo1) / tem5;
            if tem5.abs() >= 0.95 {
                tem5 = if tem5 > 0.0 { 0.95 } else { -0.95 };
            }
            eo1 += tem5;
            ktr += 1;
        }

        // Short-period periodics.
        let ecose = axnl * coseo1 + aynl * sineo1;
        let esine = axnl * sineo1 - aynl * coseo1;
        let el2 = axnl * axnl + aynl * aynl;
        let pl = am * (1.0 - el2);
        if pl < 0.0 {
            return Err(Error::Sgp4(4));
        }
        let rl = am * (1.0 - ecose);
        let rdotl = am.sqrt() * esine / rl;
        let rvdotl = pl.sqrt() / rl;
        let betal = (1.0 - el2).sqrt();
        temp = esine / (1.0 + betal);
        let sinu = am / rl * (sineo1 - aynl - axnl * temp);
        let cosu = am / rl * (coseo1 - axnl + aynl * temp);
        let mut su = sinu.atan2(cosu);
        let sin2u = (cosu + cosu) * sinu;
        let cos2u = 1.0 - 2.0 * sinu * sinu;
        temp = 1.0 / pl;
        let temp1 = 0.5 * c.j2 * temp;
        let temp2 = temp1 * temp;
        if self.deep_space {
            let cosisq = cosip * cosip;
            con41 = 3.0 * cosisq - 1.0;
            x1mth2 = 1.0 - cosisq;
            x7thm1 = 7.0 * cosisq - 1.0;
        }
        let mrt = rl * (1.0 - 1.5 * temp2 * betal * con41) + 0.5 * temp1 * x1mth2 * cos2u;
        su -= 0.25 * temp2 * x7thm1 * sin2u;
        let xnode = nodep + 1.5 * temp2 * cosip * sin2u;
        let xinc = xincp + 1.5 * temp2 * cosip * sinip * cos2u;
        let mvt = rdotl - nm * temp1 * x1mth2 * sin2u / c.xke;
        let rvdot = rvdotl + nm * temp1 * (x1mth2 * cos2u + 1.5 * con41) / c.xke;

        let (sinsu, cossu) = su.sin_cos();
        let (snod, cnod) = xnode.sin_cos();
        let (sini, cosi) = xinc.sin_cos();
        let xmx = -snod * cosi;
        let xmy = cnod * cosi;
        let ux = xmx * sinsu + cnod * cossu;
        let uy = xmy * sinsu + snod * cossu;
        let uz = sini * sinsu;
        let vx = xmx * cossu - cnod * sinsu;
        let vy = xmy * cossu - snod * sinsu;
        let vz = sini * cossu;
        if mrt < 1.0 {
            // Below the Earth's surface: the satellite has decayed.
            return Err(Error::Sgp4(6));
        }
        Ok(TemeState {
            r_km: [
                mrt * ux * c.radius_km,
                mrt * uy * c.radius_km,
                mrt * uz * c.radius_km,
            ],
            v_km_s: [
                (mvt * ux + rvdot * vx) * vkmpersec,
                (mvt * uy + rvdot * vy) * vkmpersec,
                (mvt * uz + rvdot * vz) * vkmpersec,
            ],
        })
    }

    /// The epoch as days since 1949-12-31 00:00 UTC.
    pub fn epoch_days_1950(&self) -> f64 {
        self.epoch_days_1950
    }
}

/// The values `dscom` computes that `dsinit` needs but that are not kept.
struct Dscom {
    em: f64,
    emsq: f64,
    nm: f64,
    sinim: f64,
    cosim: f64,
    s1: f64,
    s2: f64,
    s3: f64,
    s4: f64,
    s5: f64,
    ss1: f64,
    ss2: f64,
    ss3: f64,
    ss4: f64,
    ss5: f64,
    sz1: f64,
    sz3: f64,
    sz11: f64,
    sz13: f64,
    sz21: f64,
    sz23: f64,
    sz31: f64,
    sz33: f64,
    z1: f64,
    z3: f64,
    z11: f64,
    z13: f64,
    z21: f64,
    z23: f64,
    z31: f64,
    z33: f64,
}

/// The mean elements that the deep-space routines update.
struct DsState {
    em: f64,
    argpm: f64,
    inclm: f64,
    mm: f64,
    nm: f64,
    nodem: f64,
}

/// Deep-space common terms: the lunar and solar coefficients (published `dscom`).
#[allow(clippy::too_many_arguments)]
fn dscom(
    epoch: f64,
    ep: f64,
    argpp: f64,
    tc: f64,
    inclp: f64,
    nodep: f64,
    np: f64,
    ds: &mut Deep,
) -> Dscom {
    let zes = 0.01675;
    let zel = 0.05490;
    let c1ss = 2.9864797e-6;
    let c1l = 4.7968065e-7;
    let zsinis = 0.39785416;
    let zcosis = 0.91744867;
    let zcosgs = 0.1945905;
    let zsings = -0.98088458;

    let nm = np;
    let em = ep;
    let snodm = nodep.sin();
    let cnodm = nodep.cos();
    let sinomm = argpp.sin();
    let cosomm = argpp.cos();
    let sinim = inclp.sin();
    let cosim = inclp.cos();
    let emsq = em * em;
    let betasq = 1.0 - emsq;
    let rtemsq = betasq.sqrt();

    ds.peo = 0.0;
    ds.pinco = 0.0;
    ds.plo = 0.0;
    ds.pgho = 0.0;
    ds.pho = 0.0;
    let day = epoch + 18_261.5 + tc / 1440.0;
    let xnodce = fmod(4.5236020 - 9.2422029e-4 * day, TAU);
    let stem = xnodce.sin();
    let ctem = xnodce.cos();
    let zcosil = 0.91375164 - 0.03568096 * ctem;
    let zsinil = (1.0 - zcosil * zcosil).sqrt();
    let zsinhl = 0.089683511 * stem / zsinil;
    let zcoshl = (1.0 - zsinhl * zsinhl).sqrt();
    let gam = 5.8351514 + 0.0019443680 * day;
    let mut zx = 0.39785416 * stem / zsinil;
    let zy = zcoshl * ctem + 0.91744867 * zsinhl * stem;
    zx = zx.atan2(zy);
    zx = gam + zx - xnodce;
    let zcosgl = zx.cos();
    let zsingl = zx.sin();

    let mut zcosg = zcosgs;
    let mut zsing = zsings;
    let mut zcosi = zcosis;
    let mut zsini = zsinis;
    let mut zcosh = cnodm;
    let mut zsinh = snodm;
    let mut cc = c1ss;
    let xnoi = 1.0 / nm;

    // Solar terms first (lsflg 1), then lunar (lsflg 2).
    let mut sol = [0.0f64; 19];
    let (mut s1, mut s2, mut s3, mut s4, mut s5, mut s6, mut s7) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let (
        mut z1,
        mut z2,
        mut z3,
        mut z11,
        mut z12,
        mut z13,
        mut z21,
        mut z22,
        mut z23,
        mut z31,
        mut z32,
        mut z33,
    ) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for lsflg in 1..=2 {
        let a1 = zcosg * zcosh + zsing * zcosi * zsinh;
        let a3 = -zsing * zcosh + zcosg * zcosi * zsinh;
        let a7 = -zcosg * zsinh + zsing * zcosi * zcosh;
        let a8 = zsing * zsini;
        let a9 = zsing * zsinh + zcosg * zcosi * zcosh;
        let a10 = zcosg * zsini;
        let a2 = cosim * a7 + sinim * a8;
        let a4 = cosim * a9 + sinim * a10;
        let a5 = -sinim * a7 + cosim * a8;
        let a6 = -sinim * a9 + cosim * a10;

        let x1 = a1 * cosomm + a2 * sinomm;
        let x2 = a3 * cosomm + a4 * sinomm;
        let x3 = -a1 * sinomm + a2 * cosomm;
        let x4 = -a3 * sinomm + a4 * cosomm;
        let x5 = a5 * sinomm;
        let x6 = a6 * sinomm;
        let x7 = a5 * cosomm;
        let x8 = a6 * cosomm;

        z31 = 12.0 * x1 * x1 - 3.0 * x3 * x3;
        z32 = 24.0 * x1 * x2 - 6.0 * x3 * x4;
        z33 = 12.0 * x2 * x2 - 3.0 * x4 * x4;
        z1 = 3.0 * (a1 * a1 + a2 * a2) + z31 * emsq;
        z2 = 6.0 * (a1 * a3 + a2 * a4) + z32 * emsq;
        z3 = 3.0 * (a3 * a3 + a4 * a4) + z33 * emsq;
        z11 = -6.0 * a1 * a5 + emsq * (-24.0 * x1 * x7 - 6.0 * x3 * x5);
        z12 = -6.0 * (a1 * a6 + a3 * a5)
            + emsq * (-24.0 * (x2 * x7 + x1 * x8) - 6.0 * (x3 * x6 + x4 * x5));
        z13 = -6.0 * a3 * a6 + emsq * (-24.0 * x2 * x8 - 6.0 * x4 * x6);
        z21 = 6.0 * a2 * a5 + emsq * (24.0 * x1 * x5 - 6.0 * x3 * x7);
        z22 = 6.0 * (a4 * a5 + a2 * a6)
            + emsq * (24.0 * (x2 * x5 + x1 * x6) - 6.0 * (x4 * x7 + x3 * x8));
        z23 = 6.0 * a4 * a6 + emsq * (24.0 * x2 * x6 - 6.0 * x4 * x8);
        z1 = z1 + z1 + betasq * z31;
        z2 = z2 + z2 + betasq * z32;
        z3 = z3 + z3 + betasq * z33;
        s3 = cc * xnoi;
        s2 = -0.5 * s3 / rtemsq;
        s4 = s3 * rtemsq;
        s1 = -15.0 * em * s4;
        s5 = x1 * x3 + x2 * x4;
        s6 = x2 * x3 + x1 * x4;
        s7 = x2 * x4 - x1 * x3;

        if lsflg == 1 {
            sol = [
                s1, s2, s3, s4, s5, s6, s7, z1, z2, z3, z11, z12, z13, z21, z22, z23, z31, z32, z33,
            ];
            zcosg = zcosgl;
            zsing = zsingl;
            zcosi = zcosil;
            zsini = zsinil;
            zcosh = zcoshl * cnodm + zsinhl * snodm;
            zsinh = snodm * zcoshl - cnodm * zsinhl;
            cc = c1l;
        }
    }
    let [ss1, ss2, ss3, ss4, ss5, ss6, ss7, sz1, sz2, sz3, sz11, sz12, sz13, sz21, sz22, sz23, sz31, sz32, sz33] =
        sol;

    ds.zmol = fmod(4.7199672 + 0.22997150 * day - gam, TAU);
    ds.zmos = fmod(6.2565837 + 0.017201977 * day, TAU);

    // Solar terms.
    ds.se2 = 2.0 * ss1 * ss6;
    ds.se3 = 2.0 * ss1 * ss7;
    ds.si2 = 2.0 * ss2 * sz12;
    ds.si3 = 2.0 * ss2 * (sz13 - sz11);
    ds.sl2 = -2.0 * ss3 * sz2;
    ds.sl3 = -2.0 * ss3 * (sz3 - sz1);
    ds.sl4 = -2.0 * ss3 * (-21.0 - 9.0 * emsq) * zes;
    ds.sgh2 = 2.0 * ss4 * sz32;
    ds.sgh3 = 2.0 * ss4 * (sz33 - sz31);
    ds.sgh4 = -18.0 * ss4 * zes;
    ds.sh2 = -2.0 * ss2 * sz22;
    ds.sh3 = -2.0 * ss2 * (sz23 - sz21);

    // Lunar terms.
    ds.ee2 = 2.0 * s1 * s6;
    ds.e3 = 2.0 * s1 * s7;
    ds.xi2 = 2.0 * s2 * z12;
    ds.xi3 = 2.0 * s2 * (z13 - z11);
    ds.xl2 = -2.0 * s3 * z2;
    ds.xl3 = -2.0 * s3 * (z3 - z1);
    ds.xl4 = -2.0 * s3 * (-21.0 - 9.0 * emsq) * zel;
    ds.xgh2 = 2.0 * s4 * z32;
    ds.xgh3 = 2.0 * s4 * (z33 - z31);
    ds.xgh4 = -18.0 * s4 * zel;
    ds.xh2 = -2.0 * s2 * z22;
    ds.xh3 = -2.0 * s2 * (z23 - z21);

    Dscom {
        em,
        emsq,
        nm,
        sinim,
        cosim,
        s1,
        s2,
        s3,
        s4,
        s5,
        ss1,
        ss2,
        ss3,
        ss4,
        ss5,
        sz1,
        sz3,
        sz11,
        sz13,
        sz21,
        sz23,
        sz31,
        sz33,
        z1,
        z3,
        z11,
        z13,
        z21,
        z23,
        z31,
        z33,
    }
}

/// Lunar-solar periodics (published `dpper` with init = 'n').
#[allow(clippy::too_many_arguments)]
fn dpper(
    ds: &Deep,
    t: f64,
    ops: OpsMode,
    ep: &mut f64,
    inclp: &mut f64,
    nodep: &mut f64,
    argpp: &mut f64,
    mp: &mut f64,
) {
    let zns = 1.19459e-5;
    let zes = 0.01675;
    let znl = 1.5835218e-4;
    let zel = 0.05490;

    let mut zm = ds.zmos + zns * t;
    let mut zf = zm + 2.0 * zes * zm.sin();
    let mut sinzf = zf.sin();
    let mut f2 = 0.5 * sinzf * sinzf - 0.25;
    let mut f3 = -0.5 * sinzf * zf.cos();
    let ses = ds.se2 * f2 + ds.se3 * f3;
    let sis = ds.si2 * f2 + ds.si3 * f3;
    let sls = ds.sl2 * f2 + ds.sl3 * f3 + ds.sl4 * sinzf;
    let sghs = ds.sgh2 * f2 + ds.sgh3 * f3 + ds.sgh4 * sinzf;
    let shs = ds.sh2 * f2 + ds.sh3 * f3;
    zm = ds.zmol + znl * t;
    zf = zm + 2.0 * zel * zm.sin();
    sinzf = zf.sin();
    f2 = 0.5 * sinzf * sinzf - 0.25;
    f3 = -0.5 * sinzf * zf.cos();
    let sel = ds.ee2 * f2 + ds.e3 * f3;
    let sil = ds.xi2 * f2 + ds.xi3 * f3;
    let sll = ds.xl2 * f2 + ds.xl3 * f3 + ds.xl4 * sinzf;
    let sghl = ds.xgh2 * f2 + ds.xgh3 * f3 + ds.xgh4 * sinzf;
    let shll = ds.xh2 * f2 + ds.xh3 * f3;
    let pe = ses + sel - ds.peo;
    let pinc = sis + sil - ds.pinco;
    let pl = sls + sll - ds.plo;
    let mut pgh = sghs + sghl - ds.pgho;
    let mut ph = shs + shll - ds.pho;

    *inclp += pinc;
    *ep += pe;
    let sinip = inclp.sin();
    let cosip = inclp.cos();

    if *inclp >= 0.2 {
        ph /= sinip;
        pgh -= cosip * ph;
        *argpp += pgh;
        *nodep += ph;
        *mp += pl;
    } else {
        // The Lyddane modification for low inclinations.
        let sinop = nodep.sin();
        let cosop = nodep.cos();
        let mut alfdp = sinip * sinop;
        let mut betdp = sinip * cosop;
        let dalf = ph * cosop + pinc * cosip * sinop;
        let dbet = -ph * sinop + pinc * cosip * cosop;
        alfdp += dalf;
        betdp += dbet;
        *nodep = fmod(*nodep, TAU);
        if *nodep < 0.0 && ops == OpsMode::Afspc {
            *nodep += TAU;
        }
        let mut xls = *mp + *argpp + cosip * *nodep;
        let dls = pl + pgh - pinc * *nodep * sinip;
        xls += dls;
        let xnoh = *nodep;
        *nodep = alfdp.atan2(betdp);
        if *nodep < 0.0 && ops == OpsMode::Afspc {
            *nodep += TAU;
        }
        if (xnoh - *nodep).abs() > PI {
            if *nodep < xnoh {
                *nodep += TAU;
            } else {
                *nodep -= TAU;
            }
        }
        *mp += pl;
        *argpp = xls - *mp - cosip * *nodep;
    }
}

/// Deep-space initialisation: secular rates and resonance terms (published `dsinit`).
#[allow(clippy::too_many_arguments)]
fn dsinit(
    xke: f64,
    d: &Dscom,
    argpo: f64,
    t: f64,
    tc: f64,
    mo: f64,
    mdot: f64,
    no: f64,
    nodeo: f64,
    nodedot: f64,
    xpidot: f64,
    ecco: f64,
    eccsq: f64,
    st: &mut DsState,
    ds: &mut Deep,
) {
    let q22 = 1.7891679e-6;
    let q31 = 2.1460748e-6;
    let q33 = 2.2123015e-7;
    let root22 = 1.7891679e-6;
    let root44 = 7.3636953e-9;
    let root54 = 2.1765803e-9;
    let rptim = 4.37526908801129966e-3;
    let root32 = 3.7393792e-7;
    let root52 = 1.1428639e-7;
    let x2o3 = 2.0 / 3.0;
    let znl = 1.5835218e-4;
    let zns = 1.19459e-5;
    let (cosim, sinim, mut emsq) = (d.cosim, d.sinim, d.emsq);

    ds.irez = 0;
    if st.nm < 0.0052359877 && st.nm > 0.0034906585 {
        ds.irez = 1;
    }
    if (8.26e-3..=9.24e-3).contains(&st.nm) && st.em >= 0.5 {
        ds.irez = 2;
    }

    // Solar terms.
    let ses = d.ss1 * zns * d.ss5;
    let sis = d.ss2 * zns * (d.sz11 + d.sz13);
    let sls = -zns * d.ss3 * (d.sz1 + d.sz3 - 14.0 - 6.0 * emsq);
    let sghs = d.ss4 * zns * (d.sz31 + d.sz33 - 6.0);
    let mut shs = -zns * d.ss2 * (d.sz21 + d.sz23);
    if st.inclm < 5.2359877e-2 || st.inclm > PI - 5.2359877e-2 {
        shs = 0.0;
    }
    if sinim != 0.0 {
        shs /= sinim;
    }
    let sgs = sghs - cosim * shs;

    // Lunar terms.
    ds.dedt = ses + d.s1 * znl * d.s5;
    ds.didt = sis + d.s2 * znl * (d.z11 + d.z13);
    ds.dmdt = sls - znl * d.s3 * (d.z1 + d.z3 - 14.0 - 6.0 * emsq);
    let sghl = d.s4 * znl * (d.z31 + d.z33 - 6.0);
    let mut shll = -znl * d.s2 * (d.z21 + d.z23);
    if st.inclm < 5.2359877e-2 || st.inclm > PI - 5.2359877e-2 {
        shll = 0.0;
    }
    ds.domdt = sgs + sghl;
    ds.dnodt = shs;
    if sinim != 0.0 {
        ds.domdt -= cosim / sinim * shll;
        ds.dnodt += shll / sinim;
    }

    // Deep-space resonance effects.
    let theta = fmod(ds.gsto + tc * rptim, TAU);
    st.em += ds.dedt * t;
    st.inclm += ds.didt * t;
    st.argpm += ds.domdt * t;
    st.nodem += ds.dnodt * t;
    st.mm += ds.dmdt * t;

    if ds.irez != 0 {
        let aonv = (st.nm / xke).powf(x2o3);
        if ds.irez == 2 {
            // Geopotential resonance for 12-hour orbits.
            let cosisq = cosim * cosim;
            // The resonance coefficients use the epoch eccentricity.
            let emo = st.em;
            st.em = ecco;
            emsq = eccsq;
            let em = st.em;
            let eoc = em * emsq;
            let g201 = -0.306 - (em - 0.64) * 0.440;
            let (g211, g310, g322, g410, g422, g520);
            if em <= 0.65 {
                g211 = 3.616 - 13.2470 * em + 16.2900 * emsq;
                g310 = -19.302 + 117.3900 * em - 228.4190 * emsq + 156.5910 * eoc;
                g322 = -18.9068 + 109.7927 * em - 214.6334 * emsq + 146.5816 * eoc;
                g410 = -41.122 + 242.6940 * em - 471.0940 * emsq + 313.9530 * eoc;
                g422 = -146.407 + 841.8800 * em - 1629.014 * emsq + 1083.4350 * eoc;
                g520 = -532.114 + 3017.977 * em - 5740.032 * emsq + 3708.2760 * eoc;
            } else {
                g211 = -72.099 + 331.819 * em - 508.738 * emsq + 266.724 * eoc;
                g310 = -346.844 + 1582.851 * em - 2415.925 * emsq + 1246.113 * eoc;
                g322 = -342.585 + 1554.908 * em - 2366.899 * emsq + 1215.972 * eoc;
                g410 = -1052.797 + 4758.686 * em - 7193.992 * emsq + 3651.957 * eoc;
                g422 = -3581.690 + 16178.110 * em - 24462.770 * emsq + 12422.520 * eoc;
                g520 = if em > 0.715 {
                    -5149.66 + 29936.92 * em - 54087.36 * emsq + 31324.56 * eoc
                } else {
                    1464.74 - 4664.75 * em + 3763.64 * emsq
                };
            }
            let (g533, g521, g532);
            if em < 0.7 {
                g533 = -919.22770 + 4988.6100 * em - 9064.7700 * emsq + 5542.21 * eoc;
                g521 = -822.71072 + 4568.6173 * em - 8491.4146 * emsq + 5337.524 * eoc;
                g532 = -853.66600 + 4690.2500 * em - 8624.7700 * emsq + 5341.4 * eoc;
            } else {
                g533 = -37995.780 + 161616.52 * em - 229838.20 * emsq + 109377.94 * eoc;
                g521 = -51752.104 + 218913.95 * em - 309468.16 * emsq + 146349.42 * eoc;
                g532 = -40023.880 + 170470.89 * em - 242699.48 * emsq + 115605.82 * eoc;
            }
            let sini2 = sinim * sinim;
            let f220 = 0.75 * (1.0 + 2.0 * cosim + cosisq);
            let f221 = 1.5 * sini2;
            let f321 = 1.875 * sinim * (1.0 - 2.0 * cosim - 3.0 * cosisq);
            let f322 = -1.875 * sinim * (1.0 + 2.0 * cosim - 3.0 * cosisq);
            let f441 = 35.0 * sini2 * f220;
            let f442 = 39.3750 * sini2 * sini2;
            let f522 = 9.84375
                * sinim
                * (sini2 * (1.0 - 2.0 * cosim - 5.0 * cosisq)
                    + 0.33333333 * (-2.0 + 4.0 * cosim + 6.0 * cosisq));
            let f523 = sinim
                * (4.92187512 * sini2 * (-2.0 - 4.0 * cosim + 10.0 * cosisq)
                    + 6.56250012 * (1.0 + 2.0 * cosim - 3.0 * cosisq));
            let f542 = 29.53125
                * sinim
                * (2.0 - 8.0 * cosim + cosisq * (-12.0 + 8.0 * cosim + 10.0 * cosisq));
            let f543 = 29.53125
                * sinim
                * (-2.0 - 8.0 * cosim + cosisq * (12.0 + 8.0 * cosim - 10.0 * cosisq));
            let xno2 = st.nm * st.nm;
            let ainv2 = aonv * aonv;
            let mut temp1 = 3.0 * xno2 * ainv2;
            let mut temp = temp1 * root22;
            ds.d2201 = temp * f220 * g201;
            ds.d2211 = temp * f221 * g211;
            temp1 *= aonv;
            temp = temp1 * root32;
            ds.d3210 = temp * f321 * g310;
            ds.d3222 = temp * f322 * g322;
            temp1 *= aonv;
            temp = 2.0 * temp1 * root44;
            ds.d4410 = temp * f441 * g410;
            ds.d4422 = temp * f442 * g422;
            temp1 *= aonv;
            temp = temp1 * root52;
            ds.d5220 = temp * f522 * g520;
            ds.d5232 = temp * f523 * g532;
            temp = 2.0 * temp1 * root54;
            ds.d5421 = temp * f542 * g521;
            ds.d5433 = temp * f543 * g533;
            ds.xlamo = fmod(mo + nodeo + nodeo - theta - theta, TAU);
            ds.xfact = mdot + ds.dmdt + 2.0 * (nodedot + ds.dnodt - rptim) - no;
            st.em = emo;
        }
        if ds.irez == 1 {
            // Synchronous resonance terms.
            let g200 = 1.0 + emsq * (-2.5 + 0.8125 * emsq);
            let g310 = 1.0 + 2.0 * emsq;
            let g300 = 1.0 + emsq * (-6.0 + 6.60937 * emsq);
            let f220 = 0.75 * (1.0 + cosim) * (1.0 + cosim);
            let f311 = 0.9375 * sinim * sinim * (1.0 + 3.0 * cosim) - 0.75 * (1.0 + cosim);
            let mut f330 = 1.0 + cosim;
            f330 = 1.875 * f330 * f330 * f330;
            ds.del1 = 3.0 * st.nm * st.nm * aonv * aonv;
            ds.del2 = 2.0 * ds.del1 * f220 * g200 * q22;
            ds.del3 = 3.0 * ds.del1 * f330 * g300 * q33 * aonv;
            ds.del1 = ds.del1 * f311 * g310 * q31 * aonv;
            ds.xlamo = fmod(mo + nodeo + argpo - theta, TAU);
            ds.xfact = mdot + xpidot - rptim + ds.dmdt + ds.domdt + ds.dnodt - no;
        }
        st.nm = no;
    }
}

/// Deep-space secular effects and the resonance integration (published `dspace`), restarted from the epoch on every call.
fn dspace(ds: &Deep, argpo: f64, argpdot: f64, t: f64, no: f64, st: &mut DsState) {
    let fasx2 = 0.13130908;
    let fasx4 = 2.8843198;
    let fasx6 = 0.37448087;
    let g22 = 5.7686396;
    let g32 = 0.95240898;
    let g44 = 1.8014998;
    let g52 = 1.0508330;
    let g54 = 4.4108898;
    let rptim = 4.37526908801129966e-3;
    let stepp = 720.0;
    let stepn = -720.0;
    let step2 = 259_200.0;

    let theta = fmod(ds.gsto + t * rptim, TAU);
    st.em += ds.dedt * t;
    st.inclm += ds.didt * t;
    st.argpm += ds.domdt * t;
    st.nodem += ds.dnodt * t;
    st.mm += ds.dmdt * t;

    if ds.irez == 0 {
        return;
    }
    let mut atime = 0.0;
    let mut xni = no;
    let mut xli = ds.xlamo;
    let delt = if t > 0.0 { stepp } else { stepn };
    let (mut xndt, mut xldot, mut xnddt);
    let ft;
    loop {
        if ds.irez != 2 {
            xndt = ds.del1 * (xli - fasx2).sin()
                + ds.del2 * (2.0 * (xli - fasx4)).sin()
                + ds.del3 * (3.0 * (xli - fasx6)).sin();
            xldot = xni + ds.xfact;
            xnddt = ds.del1 * (xli - fasx2).cos()
                + 2.0 * ds.del2 * (2.0 * (xli - fasx4)).cos()
                + 3.0 * ds.del3 * (3.0 * (xli - fasx6)).cos();
            xnddt *= xldot;
        } else {
            let xomi = argpo + argpdot * atime;
            let x2omi = xomi + xomi;
            let x2li = xli + xli;
            xndt = ds.d2201 * (x2omi + xli - g22).sin()
                + ds.d2211 * (xli - g22).sin()
                + ds.d3210 * (xomi + xli - g32).sin()
                + ds.d3222 * (-xomi + xli - g32).sin()
                + ds.d4410 * (x2omi + x2li - g44).sin()
                + ds.d4422 * (x2li - g44).sin()
                + ds.d5220 * (xomi + xli - g52).sin()
                + ds.d5232 * (-xomi + xli - g52).sin()
                + ds.d5421 * (xomi + x2li - g54).sin()
                + ds.d5433 * (-xomi + x2li - g54).sin();
            xldot = xni + ds.xfact;
            xnddt = ds.d2201 * (x2omi + xli - g22).cos()
                + ds.d2211 * (xli - g22).cos()
                + ds.d3210 * (xomi + xli - g32).cos()
                + ds.d3222 * (-xomi + xli - g32).cos()
                + ds.d5220 * (xomi + xli - g52).cos()
                + ds.d5232 * (-xomi + xli - g52).cos()
                + 2.0
                    * (ds.d4410 * (x2omi + x2li - g44).cos()
                        + ds.d4422 * (x2li - g44).cos()
                        + ds.d5421 * (xomi + x2li - g54).cos()
                        + ds.d5433 * (-xomi + x2li - g54).cos());
            xnddt *= xldot;
        }
        if (t - atime).abs() >= stepp {
            xli += xldot * delt + xndt * step2;
            xni += xndt * delt + xnddt * step2;
            atime += delt;
        } else {
            ft = t - atime;
            break;
        }
    }
    st.nm = xni + xndt * ft + xnddt * ft * ft * 0.5;
    let xl = xli + xldot * ft + xndt * ft * ft * 0.5;
    if ds.irez != 1 {
        st.mm = xl - 2.0 * st.nodem + 2.0 * theta;
    } else {
        st.mm = xl - st.nodem - st.argpm + theta;
    }
    let dndt = st.nm - no;
    st.nm = no + dndt;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gmst_of_vallado_example_3_5() {
        // Vallado, Fundamentals of Astrodynamics and Applications, example 3-5: at 1992-08-20 12:14:00 UT1,
        // GMST = 152.578 787 810°.
        let jd = 2_448_854.5 + (12.0 + 14.0 / 60.0) / 24.0;
        assert!(
            (gstime(jd).to_degrees() - 152.578_787_810).abs() < 1e-7,
            "{}",
            gstime(jd).to_degrees()
        );
    }

    #[test]
    fn refuses_bad_elements() {
        let mut e = Elements {
            epoch_days_1950: 20_000.0,
            bstar: 0.0,
            eccentricity: 0.001,
            arg_perigee_rad: 0.0,
            inclination_rad: 0.9,
            mean_anomaly_rad: 0.0,
            mean_motion_rad_min: 0.06,
            raan_rad: 0.0,
        };
        assert!(Sgp4::new(&e, Gravity::Wgs72, OpsMode::Improved).is_ok());
        e.eccentricity = 1.0;
        assert_eq!(
            Sgp4::new(&e, Gravity::Wgs72, OpsMode::Improved).err(),
            Some(Error::Sgp4(1))
        );
        e.eccentricity = 0.001;
        e.mean_motion_rad_min = 0.0;
        assert_eq!(
            Sgp4::new(&e, Gravity::Wgs72, OpsMode::Improved).err(),
            Some(Error::Sgp4(2))
        );
        e.mean_motion_rad_min = f64::NAN;
        assert_eq!(
            Sgp4::new(&e, Gravity::Wgs72, OpsMode::Improved).err(),
            Some(Error::NotFinite)
        );
        e.mean_motion_rad_min = 0.06;
        let s = Sgp4::new(&e, Gravity::Wgs84, OpsMode::Afspc).unwrap();
        assert!(s.propagate(f64::INFINITY).is_err());
        assert!(!s.is_deep_space());
        assert!((s.period_min() - TAU / 0.06).abs() < 1.0);
    }
}
