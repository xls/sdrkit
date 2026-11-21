//! The receiver core on the generator (no graph): a GPS fix from a warm start.

use sdrkit_core::Complex32;
use sdrkit_stack_gnss::core::{CoreConfig, GnssCore};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator};
use sdrkit_stack_gnss::geo;
use sdrkit_stack_gnss::system::System;

#[test]
fn gps_fix_from_the_generator() {
    let fs = 4e6;
    let mut g = GnssGenerator::new(GenOptions::default());
    let mut sky = crate::Sky::new(&g);
    let mut rx = GnssCore::new(CoreConfig {
        systems: System::Gps.bit(),
        ..Default::default()
    });
    rx.configure(fs, 1575.42e6, false).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
    let n = (34.0 * fs) as usize / x.len();
    for _ in 0..n {
        sky.render(&mut g, &mut x);
        rx.feed(&x);
    }
    for l in rx.log.iter() {
        eprintln!("{l}");
    }
    let fix = rx.fix.clone().expect("a fix");
    let truth = g.receiver();
    let d = geo::dist(fix.ecef, truth);
    let tg = geo::ecef_to_geodetic(truth);
    let enu = geo::ecef_to_enu(
        tg,
        [
            fix.ecef[0] - truth[0],
            fix.ecef[1] - truth[1],
            fix.ecef[2] - truth[2],
        ],
    );
    eprintln!("fix {:?} error {d:.2} m enu {enu:?}", fix.type_text);
    assert!((enu[0].powi(2) + enu[1].powi(2)).sqrt() < 8.0, "{enu:?}");
    assert!(enu[2].abs() < 15.0);
}
