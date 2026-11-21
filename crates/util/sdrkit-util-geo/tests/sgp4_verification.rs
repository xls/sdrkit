//! SGP4 against the published verification set of Vallado, Crawford, Hujsak and Kelso, "Revisiting Spacetrack Report #3",
//! AIAA 2006-6753: the 33 element sets of `SGP4-VER.TLE` and the published positions and velocities at every listed time
//! (WGS-72, AFSPC mode). `tests/data/sgp4-ver.out` holds the time, position (km) and velocity (km/s) columns of the published
//! output (`tmatverDec2015.out`), with a `<satellite> xx` line before each set and without the stray last line of that file.

use sdrkit_util_geo::orbit::sgp4::{Elements, Gravity, OpsMode, Sgp4};
use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
use sdrkit_util_geo::Error;

struct Case {
    tle: Tle,
    stop_min: f64,
    step_min: f64,
}

fn cases() -> Vec<Case> {
    let text = include_str!("data/sgp4-ver.tle");
    let lines: Vec<&str> = text
        .lines()
        .filter(|l| l.starts_with('1') || l.starts_with('2'))
        .collect();
    lines
        .chunks(2)
        .map(|p| {
            let extra: Vec<f64> = p[1][69..]
                .split_whitespace()
                .map(|x| x.parse().unwrap())
                .collect();
            Case {
                tle: Tle::from_lines(None, &p[0][..69], &p[1][..69], Checksum::Ignore).unwrap(),
                stop_min: extra[1],
                step_min: extra[2],
            }
        })
        .collect()
}

fn expected() -> Vec<(u32, Vec<[f64; 7]>)> {
    let mut out: Vec<(u32, Vec<[f64; 7]>)> = Vec::new();
    for line in include_str!("data/sgp4-ver.out").lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() == 2 && f[1] == "xx" {
            out.push((f[0].parse().unwrap(), Vec::new()));
        } else if f.len() == 7 {
            let mut row = [0.0; 7];
            for (r, x) in row.iter_mut().zip(&f) {
                *r = x.parse().unwrap();
            }
            out.last_mut().unwrap().1.push(row);
        }
    }
    out
}

#[test]
fn every_published_case() {
    let cases = cases();
    let expected = expected();
    assert_eq!(cases.len(), 33);
    assert_eq!(expected.len(), 33);
    let (mut worst_r, mut worst_v, mut points) = (0.0f64, 0.0f64, 0);
    for (case, (satnum, rows)) in cases.iter().zip(&expected) {
        assert_eq!(case.tle.catalog_number, *satnum);
        let sgp4 = match Sgp4::new(
            &Elements::from_tle(&case.tle),
            Gravity::Wgs72,
            OpsMode::Afspc,
        ) {
            Ok(s) => s,
            Err(e) => {
                // Satellite 33334 is refused at initialisation; the published file repeats the previous satellite's last state.
                assert_eq!(*satnum, 33_334, "{satnum}: {e:?}");
                assert!(matches!(e, Error::Sgp4(_)));
                continue;
            }
        };
        for row in rows {
            let s = sgp4
                .propagate(row[0])
                .unwrap_or_else(|e| panic!("{satnum} at {} min: {e:?}", row[0]));
            for k in 0..3 {
                let dr = (s.r_km[k] - row[1 + k]).abs();
                let dv = (s.v_km_s[k] - row[4 + k]).abs();
                worst_r = worst_r.max(dr);
                worst_v = worst_v.max(dv);
                // The published values carry 8 decimals of km and 9 of km/s; the port agrees to within that rounding.
                assert!(
                    dr < 1e-7,
                    "{satnum} at {} min: r[{k}] {} vs {}",
                    row[0],
                    s.r_km[k],
                    row[1 + k]
                );
                assert!(
                    dv < 1e-9,
                    "{satnum} at {} min: v[{k}] {} vs {}",
                    row[0],
                    s.v_km_s[k],
                    row[4 + k]
                );
            }
            points += 1;
        }
        // Where the published output stops before the requested end, the next time step must be refused (decay, or the
        // eccentricity leaving its range).
        let last = rows.last().unwrap()[0];
        if last + 1e-9 < case.stop_min {
            let next = (last + case.step_min).min(case.stop_min);
            assert!(
                sgp4.propagate(next).is_err(),
                "{satnum} should fail at {next} min"
            );
        }
    }
    assert!(points > 500, "{points}");
    eprintln!("SGP4 verification: {points} states, worst position error {worst_r:.2e} km, worst velocity error {worst_v:.2e} km/s");
}

#[test]
fn improved_mode_matches_for_the_near_earth_cases() {
    // The two modes differ only in the low-inclination Lyddane branch of the deep-space periodics.
    for case in cases() {
        let el = Elements::from_tle(&case.tle);
        let (Ok(a), Ok(i)) = (
            Sgp4::new(&el, Gravity::Wgs72, OpsMode::Afspc),
            Sgp4::new(&el, Gravity::Wgs72, OpsMode::Improved),
        ) else {
            continue;
        };
        if a.is_deep_space() {
            continue;
        }
        let (sa, si) = (a.propagate(360.0), i.propagate(360.0));
        assert_eq!(sa.is_ok(), si.is_ok());
        if let (Ok(sa), Ok(si)) = (sa, si) {
            assert_eq!(sa.r_km, si.r_km);
        }
    }
}
