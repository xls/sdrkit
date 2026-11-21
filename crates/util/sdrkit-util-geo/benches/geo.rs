//! Throughput of the hot paths, timed with `sdrkit_core::bench::measure` (the fastest of several runs). Run with
//! `cargo bench -p sdrkit-util-geo`; each line prints calls per second.

use sdrkit_core::bench::measure;
use sdrkit_util_geo::coord::{format_ddmm, Axis};
use sdrkit_util_geo::cpr::{decode_global, encode, CprFormat};
use sdrkit_util_geo::nmea::{self, aivdm, Gga};
use sdrkit_util_geo::orbit::tle::{Checksum, Tle};
use sdrkit_util_geo::orbit::{Satellite, Sgp4};
use sdrkit_util_geo::time;
use sdrkit_util_geo::{maidenhead, Ellipsoid, Observer, Position};
use std::hint::black_box;

const N: usize = 10_000;

fn report(name: &str, items: usize, f: impl FnMut()) {
    let r = measure(0.5, items, 0.0, f);
    println!(
        "{name:<32} {:>12.0} calls/s  ({:.1} ns/call)",
        r.items_per_second,
        1e9 / r.items_per_second
    );
}

fn main() {
    let pts: Vec<Position> = (0..N)
        .map(|i| Position::new(-60.0 + (i % 120) as f64, -170.0 + (i % 340) as f64))
        .collect();
    let here = Position::new(52.52, 13.405);
    let obs = Observer::new(here);

    report("haversine distance", N, || {
        for p in &pts {
            black_box(here.distance_m(p));
        }
    });
    report("vincenty inverse", N, || {
        for p in &pts {
            black_box(Ellipsoid::WGS84.inverse(&here, p).ok());
        }
    });
    let ecef: Vec<_> = pts.iter().map(|p| p.to_ecef()).collect();
    report("ecef to geodetic", N, || {
        for e in &ecef {
            black_box(e.to_position().unwrap());
        }
    });
    report("look angles", N, || {
        for e in &ecef {
            black_box(obs.look(e));
        }
    });
    report("maidenhead encode 6", N, || {
        for p in &pts {
            black_box(maidenhead::encode(p, 6).unwrap());
        }
    });
    report("ddmm format", N, || {
        for p in &pts {
            black_box(format_ddmm(p.lat_deg, Axis::Latitude, 4).unwrap());
        }
    });
    let f = CprFormat::AIRBORNE;
    let frames: Vec<_> = pts
        .iter()
        .map(|p| (encode(&f, p, false).unwrap(), encode(&f, p, true).unwrap()))
        .collect();
    report("cpr global decode", N, || {
        for (e, o) in &frames {
            black_box(decode_global(&f, e, o, true, None).ok());
        }
    });
    let iss = Sgp4::from_tle(
        &Tle::from_lines(
            None,
            "1 25544U 98067A   08264.51782528 -.00002182  00000-0 -11606-4 0  2927",
            "2 25544  51.6416 247.4627 0006703 130.5360 325.0288 15.72125391563537",
            Checksum::Require,
        )
        .unwrap(),
    )
    .unwrap();
    let molniya = Sgp4::from_tle(
        &Tle::from_lines(
            None,
            "1 08195U 75081A   06176.33215444  .00000099  00000-0  11873-3 0   813",
            "2 08195  64.1586 279.0717 6877146 264.7651  20.2257  2.00491383225656",
            Checksum::Require,
        )
        .unwrap(),
    )
    .unwrap();
    report("sgp4 near-earth", N, || {
        for i in 0..N {
            black_box(iss.propagate(i as f64).unwrap());
        }
    });
    report("sdp4 12-h resonant (1 day out)", N, || {
        for i in 0..N {
            black_box(molniya.propagate(1440.0 + i as f64 * 1e-3).unwrap());
        }
    });
    let sat = Satellite::new(iss.clone());
    let t0 = iss.epoch_unix();
    report("passes over one day", 1, || {
        black_box(sat.passes(&obs, t0, t0 + 86_400.0, 0.0).count());
    });
    report("nmea parse + gga", N, || {
        for _ in 0..N {
            let s = nmea::parse(black_box(
                "$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,*47",
            ))
            .unwrap();
            black_box(Gga::from_sentence(&s).unwrap());
        }
    });
    let mut bits = Vec::with_capacity(512);
    report("aivdm dearmour 168 bits", N, || {
        for _ in 0..N {
            bits.clear();
            aivdm::dearmour(black_box("177KQJ5000G?tO`K>RA1wUbN0TKH"), 0, &mut bits).unwrap();
        }
    });
    report("iso8601 format", N, || {
        for i in 0..N {
            black_box(time::iso8601(1.794e9 + i as f64, 3).unwrap());
        }
    });
}
