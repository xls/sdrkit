//! NMEA 0183 RMC and GGA sentences of a fix, and RINEX 3.04 observation and GPS navigation text.

use crate::core::{Fix, MeasEpoch};
use crate::orbit::Kepler;
use crate::system::{System, C, GPS_PI, WEEK_S};
use sdrkit_util_geo::coord::{format_ddmm, Axis};
use sdrkit_util_geo::nmea::{date_field, time_field, Builder};
use sdrkit_util_geo::time::gnss::GPS_EPOCH_UNIX;
use sdrkit_util_geo::time::Civil;
use sdrkit_util_geo::KNOT_M_S;
use std::collections::BTreeSet;

/// One NMEA sentence: the address, then the fields; `*hh` is added.
fn sentence(address: &str, fields: &[&str]) -> Result<String, sdrkit_util_geo::Error> {
    let mut b = Builder::new('$', address)?;
    for f in fields {
        b.field(f)?;
    }
    Ok(b.finish()?.as_str().to_string())
}

/// `$GPRMC` and `$GPGGA` of a valid fix (NMEA 0183 version 4.11). Nothing while UTC is not known.
pub fn nmea(fix: &Fix) -> Vec<String> {
    let Some(utc) = fix.utc.filter(|_| fix.valid) else {
        return Vec::new();
    };
    let make = || -> Result<Vec<String>, sdrkit_util_geo::Error> {
        // The time is rounded before it is split, so the seconds carry (shared rule of sdrkit-util-geo).
        let time = time_field(utc, 3)?;
        let date = date_field(utc)?;
        let (la, ns) = format_ddmm(fix.lat, Axis::Latitude, 4)?;
        let (lo, ew) = format_ddmm(fix.lon, Axis::Longitude, 4)?;
        let (kn, course) = if fix.has_velocity {
            (fix.speed / KNOT_M_S, fix.course.unwrap_or(0.0))
        } else {
            (0.0, 0.0)
        };
        let (ns, ew) = (ns.to_string(), ew.to_string());
        let kn = format!("{kn:.1}");
        let course = format!("{course:.1}");
        let rmc = sentence(
            "GPRMC",
            &[
                time.as_str(),
                "A",
                la.as_str(),
                &ns,
                lo.as_str(),
                &ew,
                &kn,
                &course,
                date.as_str(),
                "",
                "",
                "A",
            ],
        )?;
        let hdop = if fix.hdop == 0.0 { 99.9 } else { fix.hdop };
        let sats = format!("{:02}", fix.satellites.min(12));
        let hdop = format!("{hdop:.1}");
        let height = format!("{:.1}", fix.height);
        let gga = sentence(
            "GPGGA",
            &[
                time.as_str(),
                la.as_str(),
                &ns,
                lo.as_str(),
                &ew,
                "1",
                &sats,
                &hdop,
                &height,
                "M",
                "0.0",
                "M",
                "",
                "",
            ],
        )?;
        Ok(vec![rmc, gga])
    };
    make().unwrap_or_default()
}

/// The calendar of GPS week `w` and seconds `s` on the GPS scale, with the seconds of the minute. The fraction of the second is
/// rounded to `decimals` first, so the seconds carry into the minute. `None` for a time the calendar cannot hold.
fn calendar(w: i64, s: f64, decimals: i32) -> Option<(Civil, f64)> {
    let scale = 10f64.powi(decimals);
    let whole = s.floor();
    let mut frac = ((s - whole) * scale).round() / scale;
    let mut secs = GPS_EPOCH_UNIX + w * WEEK_S as i64 + whole as i64;
    if frac >= 1.0 {
        secs += 1;
        frac -= 1.0;
    }
    let c = Civil::from_unix(secs, 0).ok()?;
    let second = c.second as f64 + frac;
    Some((c, second))
}

/// The RINEX signal-strength digit of a C/N0.
pub fn ssi(cn0: f64) -> u8 {
    if cn0 < 12.0 {
        1
    } else if cn0 >= 54.0 {
        9
    } else {
        (((cn0 - 12.0) / 6.0).floor() as u8 + 2).min(9)
    }
}

/// The RINEX observation writer (code pseudorange, Doppler and signal strength).
#[derive(Clone, Debug)]
pub struct RinexObs {
    pub program: String,
    pub marker: String,
    pub run_date: String,
    pub approx: [f64; 3],
    first: Option<(f64, f64, i64)>,
    clock_offset: f64,
    systems: BTreeSet<char>,
    first_obs: Option<(i64, f64)>,
    /// Receiver-clock steps made, each with the change of the mean travel time (ms).
    pub steps: Vec<f64>,
    pub travel_ms: Vec<f64>,
}

impl RinexObs {
    pub fn new(program: &str, marker: &str, run_date: &str) -> RinexObs {
        RinexObs {
            program: program.into(),
            marker: marker.into(),
            run_date: run_date.into(),
            approx: [0.0; 3],
            first: None,
            clock_offset: 0.0,
            systems: BTreeSet::new(),
            first_obs: None,
            steps: Vec::new(),
            travel_ms: Vec::new(),
        }
    }

    /// The transmit time on the GPS scale (BeiDou + 14 s), and whether the system is written.
    fn gps_scale(sys: System, tx: f64) -> Option<f64> {
        match sys {
            System::Gps | System::Qzss | System::Galileo | System::Sbas => Some(tx),
            System::Beidou => Some(tx + 14.0),
            System::Glonass => None,
        }
    }

    /// Writes one epoch (`week`: the GPS week of the transmit times). Returns the lines, or nothing when no satellite is written.
    pub fn epoch(&mut self, ep: &MeasEpoch, week: i64) -> Vec<String> {
        let mut rows: Vec<(System, u16, f64, f64, f64)> = ep
            .sats
            .iter()
            .filter_map(|m| {
                Self::gps_scale(m.sat.system, m.tx_time)
                    .map(|t| (m.sat.system, m.sat.prn, t, m.doppler, m.cn0))
            })
            .collect();
        if rows.is_empty() {
            return Vec::new();
        }
        let order = |s: System| match s {
            System::Gps => 0,
            System::Glonass => 1,
            System::Galileo => 2,
            System::Qzss => 3,
            System::Beidou => 4,
            System::Sbas => 5,
        };
        rows.sort_by_key(|r| (order(r.0), r.1));
        // Receiver clock: the first epoch 70 ms after the latest transmit time (whole ms), then the sample count.
        let latest = rows.iter().map(|r| r.2).fold(f64::NEG_INFINITY, f64::max);
        // The receive clock runs on from the first epoch, in the week of that epoch.
        let (sig0, rx0, week0) = *self.first.get_or_insert_with(|| {
            let rx = ((latest + 0.070) * 1e3).round() / 1e3;
            (ep.rx_time, rx, week)
        });
        let mut rx = rx0 + (ep.rx_time - sig0) + self.clock_offset;
        let mean_travel = rows
            .iter()
            .map(|r| crate::geo::wrap_week(rx - r.2))
            .sum::<f64>()
            / rows.len() as f64;
        let mt_ms = mean_travel * 1e3;
        if !(60.0..=95.0).contains(&mt_ms) {
            let target = if mt_ms > 95.0 { 75.0 } else { 80.0 };
            let step = (mt_ms - target).round();
            self.clock_offset -= step * 1e-3;
            rx -= step * 1e-3;
            self.steps.push(-step);
        }
        let mean2 = rows
            .iter()
            .map(|r| crate::geo::wrap_week(rx - r.2))
            .sum::<f64>()
            / rows.len() as f64;
        self.travel_ms.push(mean2 * 1e3);
        let mut w = week0;
        let mut sow = rx;
        while sow >= WEEK_S {
            sow -= WEEK_S;
            w += 1;
        }
        while sow < 0.0 {
            sow += WEEK_S;
            w -= 1;
        }
        let Some((cal, second)) = calendar(w, sow, 7) else {
            return Vec::new();
        };
        if self.first_obs.is_none() {
            self.first_obs = Some((w, sow));
        }
        let mut out = Vec::with_capacity(rows.len() + 1);
        out.push(format!(
            "> {:04} {:02} {:02} {:02} {:02}{:11.7}  0{:3}",
            cal.year,
            cal.month,
            cal.day,
            cal.hour,
            cal.minute,
            second,
            rows.len()
        ));
        for (sys, prn, tx, dop, cn0) in rows {
            self.systems.insert(sys.letter());
            // A transmit time from the other side of a week boundary is wrapped.
            let pr = C * crate::geo::wrap_week(rx - tx);
            let n = match sys {
                System::Qzss => prn - 192,
                System::Sbas => prn - 100,
                _ => prn,
            };
            let d = ssi(cn0);
            out.push(format!(
                "{}{:02}{:14.3} {}{:14.3} {}{:14.3} {}",
                sys.letter(),
                n,
                pr,
                d,
                dop,
                d,
                cn0,
                d
            ));
        }
        out
    }

    /// The header, valid once the first epoch is written.
    pub fn header(&self) -> Vec<String> {
        let lab = |body: String, label: &str| format!("{:<60}{}", body, label);
        let mut h = vec![
            lab(
                format!("{:9.2}{:11}{:<20}{:<20}", 3.04, "", "OBSERVATION DATA", "M"),
                "RINEX VERSION / TYPE",
            ),
            lab(
                format!("{:<20}{:<20}{:<20}", self.program, "", self.run_date),
                "PGM / RUN BY / DATE",
            ),
            lab(format!("{:<60}", self.marker), "MARKER NAME"),
            lab(
                format!(
                    "{:14.4}{:14.4}{:14.4}",
                    self.approx[0], self.approx[1], self.approx[2]
                ),
                "APPROX POSITION XYZ",
            ),
        ];
        for s in &self.systems {
            let code = match s {
                'C' => ["C2I", "D2I", "S2I"],
                'E' => ["C1B", "D1B", "S1B"],
                _ => ["C1C", "D1C", "S1C"],
            };
            h.push(lab(
                format!("{}  {:3} {} {} {}", s, 3, code[0], code[1], code[2]),
                "SYS / # / OBS TYPES",
            ));
        }
        if let Some((c, second)) = self.first_obs.and_then(|(w, s)| calendar(w, s, 7)) {
            h.push(lab(
                format!(
                    "{:6}{:6}{:6}{:6}{:6}{:13.7}     GPS",
                    c.year, c.month, c.day, c.hour, c.minute, second
                ),
                "TIME OF FIRST OBS",
            ));
        }
        h.push(lab(String::new(), "END OF HEADER"));
        h
    }
}

fn d19(v: f64) -> String {
    // RINEX's D19.12: one digit before the point is written by `{:e}`; the field is right aligned to 19.
    let s = format!("{:.12E}", v);
    let (m, e) = s.split_once('E').expect("the exponent form has an E");
    let e: i32 = e.parse().expect("the exponent is a number");
    format!(
        "{:>19}",
        format!("{}E{}{:02}", m, if e < 0 { '-' } else { '+' }, e.abs())
    )
}

/// A RINEX 3.04 GPS navigation record of an ephemeris (full week of toe).
pub fn rinex_nav(e: &Kepler) -> Vec<String> {
    let Some((c, second)) = calendar(e.week, e.toc, 0) else {
        return Vec::new();
    };
    let sc = |v: f64| v / GPS_PI * GPS_PI;
    let mut out = vec![format!(
        "G{:02} {:04} {:02} {:02} {:02} {:02} {:02}{}{}{}",
        e.prn,
        c.year,
        c.month,
        c.day,
        c.hour,
        c.minute,
        second as u32,
        d19(e.af0),
        d19(e.af1),
        d19(e.af2)
    )];
    let rows = [
        [e.iode as f64, e.crs, e.delta_n, sc(e.m0)],
        [e.cuc, e.e, e.cus, e.sqrt_a],
        [e.toe, e.cic, e.omega0, e.cis],
        [e.i0, e.crc, e.omega, e.omega_dot],
        [e.idot, 1.0, e.week as f64, 0.0],
        [e.ura as f64, e.health as f64, e.tgd, e.iodc as f64],
        [e.toe, e.fit as f64, 0.0, 0.0],
    ];
    for r in rows {
        out.push(format!(
            "    {}{}{}{}",
            d19(r[0]),
            d19(r[1]),
            d19(r[2]),
            d19(r[3])
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SatMeas;
    use crate::system::SatId;

    fn m(sys: System, prn: u16, tx: f64, dop: f64, cn0: f64) -> SatMeas {
        SatMeas {
            sat: SatId::new(sys, prn),
            glo_k: None,
            tx_time: tx,
            doppler: dop,
            carrier: 1575.42e6,
            cn0,
            used: false,
            residual: None,
        }
    }

    #[test]
    fn calendar_of_gps_time() {
        let (c, _) = calendar(2000, 0.0, 0).unwrap();
        assert_eq!((c.year, c.month, c.day, c.hour), (2018, 5, 6, 0));
        let (c, sec) = calendar(2400, 302_461.25, 7).unwrap();
        assert_eq!(
            (c.year, c.month, c.day, c.hour, c.minute),
            (2026, 1, 7, 12, 1)
        );
        assert!((sec - 1.25).abs() < 1e-9);
        let (c, _) = calendar(2400, 604_800.0, 0).unwrap();
        assert_eq!((c.year, c.month, c.day), (2026, 1, 11));
    }

    #[test]
    fn rinex_seconds_carry_into_the_minute() {
        // 59.99999996 s rounds to 60.0000000 at seven decimals: the epoch is the next minute at 0 s, never "60.0000000".
        let (c, sec) = calendar(2400, 302_459.999_999_96, 7).unwrap();
        assert_eq!((c.hour, c.minute), (12, 1));
        assert_eq!(format!("{sec:11.7}"), "  0.0000000");
    }

    #[test]
    fn nmea_time_and_minutes_carry() {
        // UTC 12:00:59.9996 at three decimals is 12:01:00.000, and 25.999999999 degrees is 26 degrees 0 minutes.
        let f = Fix {
            valid: true,
            lat: 25.999_999_999,
            lon: -55.999_999_999,
            utc: Some(crate::geo::utc_unix(2400, 302_459.999_6 + 18.0, 18)),
            ..Default::default()
        };
        let s = nmea(&f);
        assert!(
            s[0].starts_with("$GPRMC,120100.000,A,2600.0000,N,05600.0000,W,"),
            "{}",
            s[0]
        );
    }

    #[test]
    fn an_epoch_and_the_header() {
        let t0 = 302_400.0;
        let mut w = RinexObs::new("sdrkit", "MARKER", "20260107 120000 UTC");
        let ep = MeasEpoch {
            rx_time: 1.0,
            sats: vec![
                m(System::Gps, 12, t0 - 0.007, 100.0, 40.0),
                m(System::Gps, 5, t0, -1234.5, 40.0),
                m(System::Beidou, 19, t0 - 0.012 - 14.0, 0.0, 33.0),
            ],
            gps_time: None,
        };
        let lines = w.epoch(&ep, 2400);
        assert_eq!(lines[0].len(), 35, "{}", lines[0]);
        assert!(lines[0].ends_with("  3"));
        assert!(
            lines[1].starts_with("G05")
                && lines[2].starts_with("G12")
                && lines[3].starts_with("C19")
        );
        let pr = |l: &str| l[3..17].trim().parse::<f64>().unwrap();
        assert!((pr(&lines[1]) - C * 0.070).abs() < 0.05);
        assert!((pr(&lines[2]) - pr(&lines[1]) - C * 0.007).abs() < 0.05);
        assert!((pr(&lines[3]) - pr(&lines[1]) - C * 0.012).abs() < 0.05);
        assert_eq!(lines[1].len(), 3 + 3 * 16);
        assert_eq!(&lines[1][18..19], "6");
        assert_eq!(&lines[3][18..19], "5");
        assert!(lines[1][19..33].trim() == "-1234.500");
        assert!(lines[3][35..49].trim() == "33.000");
        let h = w.header();
        assert!(h[0].starts_with("     3.04"));
        assert_eq!(&h[0][20..36], "OBSERVATION DATA");
        assert_eq!(&h[0][40..41], "M");
        assert!(h.iter().all(|l| l.len() <= 80));
        assert!(h
            .iter()
            .any(|l| l.starts_with('G') && l.contains("SYS / # / OBS TYPES")));
        assert!(h
            .iter()
            .any(|l| l.starts_with('C') && l.contains("SYS / # / OBS TYPES")));
        assert!(!h
            .iter()
            .any(|l| l.starts_with('E') && l.contains("SYS / # / OBS TYPES")));
        assert!(h.iter().any(|l| l.ends_with("TIME OF FIRST OBS")));
        assert!(h.last().unwrap().ends_with("END OF HEADER"));
        // GLONASS only writes nothing.
        let mut w2 = RinexObs::new("sdrkit", "M", "");
        let g = MeasEpoch {
            rx_time: 1.0,
            sats: vec![m(System::Glonass, 3, 1000.0, 0.0, 40.0)],
            gps_time: None,
        };
        assert!(w2.epoch(&g, 2400).is_empty());
    }

    #[test]
    fn clock_steps_and_week_wrap() {
        let mut w = RinexObs::new("sdrkit", "M", "");
        // A sample clock 300 ppm slow: the travel time drifts 0.3 ms a second.
        for k in 0..120 {
            let t = k as f64;
            let tx = 100_000.0 + t * (1.0 + 300e-6);
            let ep = MeasEpoch {
                rx_time: t,
                sats: vec![
                    m(System::Gps, 1, tx, 0.0, 40.0),
                    m(System::Gps, 2, tx - 0.004, 0.0, 40.0),
                ],
                gps_time: None,
            };
            w.epoch(&ep, 2400);
        }
        assert!(w.travel_ms.iter().all(|t| (59.0..=96.0).contains(t)));
        assert!(!w.steps.is_empty() && w.steps.len() <= 2, "{:?}", w.steps);
        for s in &w.steps {
            assert!((s.abs() - 20.0).abs() <= 1.5, "{s}");
        }
        // Week wrap: the same pseudorange either side.
        let mut w = RinexObs::new("sdrkit", "M", "");
        let mut prs = Vec::new();
        for k in 0..3 {
            let t = k as f64;
            let tx = WEEK_S - 1.5 + t;
            let tx = if tx >= WEEK_S { tx - WEEK_S } else { tx };
            let ep = MeasEpoch {
                rx_time: t,
                sats: vec![m(System::Gps, 1, tx, 0.0, 40.0)],
                gps_time: None,
            };
            let l = w.epoch(&ep, if k == 2 { 2401 } else { 2400 });
            prs.push(l[1][3..17].trim().parse::<f64>().unwrap());
            if k == 2 {
                assert!(l[0].starts_with("> 2026 01 11"), "{}", l[0]);
            }
        }
        assert!((prs[0] - prs[2]).abs() < 1e-3, "{prs:?}");
    }

    #[test]
    fn nmea_sentences_have_checksums() {
        let mut f = Fix {
            valid: true,
            lat: 25.2,
            lon: 55.36,
            height: 10.0,
            hdop: 1.2,
            satellites: 8,
            ..Default::default()
        };
        f.utc = Some(crate::geo::utc_unix(2400, 302_461.25, 18));
        let s = nmea(&f);
        assert!(s[0].starts_with("$GPRMC,") && s[1].starts_with("$GPGGA,"));
        assert!(s[0].contains(",2512.0000,N,05521.6000,E,"));
        for l in &s {
            let (body, cs) = l[1..].split_once('*').unwrap();
            let c = body.bytes().fold(0u8, |a, b| a ^ b);
            assert_eq!(format!("{c:02X}"), cs);
        }
        let nav = rinex_nav(&Kepler {
            prn: 5,
            week: 2400,
            toc: 352_800.0,
            ..Default::default()
        });
        assert_eq!(nav.len(), 8);
        assert!(nav[0].starts_with("G05 2026"));
    }
}
