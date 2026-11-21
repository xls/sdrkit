//! The long whole-receiver cases of the other systems. Each runs one to two minutes of signal, so they are
//! ignored by default: `cargo test -p sdrkit-stack-gnss --release -- --ignored`.

use sdrkit_core::Complex32;
use sdrkit_stack_gnss::codes::E1bTable;
use sdrkit_stack_gnss::core::{CoreConfig, GnssCore};
use sdrkit_stack_gnss::gen::{GenOptions, GnssGenerator, GALILEO_TABLE_SEED};
use sdrkit_stack_gnss::geo;
use sdrkit_stack_gnss::system::System;

fn run(opt: GenOptions, cfg: CoreConfig, secs: f64) -> (GnssCore, GnssGenerator) {
    let fs = opt.rate_hz;
    let centre = opt.centre_hz;
    let mut g = GnssGenerator::new(opt);
    let mut sky = crate::Sky::new(&g);
    let mut rx = GnssCore::new(cfg);
    rx.configure(fs, centre, false).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); 65_536];
    for _ in 0..(secs * fs) as usize / x.len() {
        sky.render(&mut g, &mut x);
        rx.feed(&x);
    }
    for l in rx.log.iter().rev().take(12) {
        eprintln!("{l}");
    }
    (rx, g)
}

fn errors(rx: &GnssCore, g: &GnssGenerator) -> (f64, f64) {
    let f = rx.fix.as_ref().expect("a fix");
    let t = g.receiver();
    let enu = geo::ecef_to_enu(
        geo::ecef_to_geodetic(t),
        [f.ecef[0] - t[0], f.ecef[1] - t[1], f.ecef[2] - t[2]],
    );
    eprintln!("{}: error {enu:?}", f.type_text);
    ((enu[0].powi(2) + enu[1].powi(2)).sqrt(), enu[2].abs())
}

#[test]
#[ignore]
fn gps_qzss_sbas() {
    let (rx, g) = run(
        GenOptions {
            systems: System::Gps.bit() | System::Qzss.bit() | System::Sbas.bit(),
            ..Default::default()
        },
        CoreConfig {
            systems: System::Gps.bit() | System::Qzss.bit() | System::Sbas.bit(),
            week_ref: 2400,
            ..Default::default()
        },
        55.0,
    );
    let (h, v) = errors(&rx, &g);
    assert!(h < 12.0 && v < 20.0);
    let f = rx.fix.as_ref().unwrap();
    assert!(f.type_text.contains("QZSS"), "{}", f.type_text);
    let sbas = rx
        .channels
        .iter()
        .filter(|c| c.sat.system == System::Sbas)
        .count();
    assert_eq!(sbas, 3);
}

#[test]
#[ignore]
fn gps_galileo() {
    let (rx, g) = run(
        GenOptions {
            systems: System::Gps.bit() | System::Galileo.bit(),
            ..Default::default()
        },
        CoreConfig {
            systems: System::Gps.bit() | System::Galileo.bit(),
            week_ref: 2400,
            galileo: Some(E1bTable::synthetic(GALILEO_TABLE_SEED, 36)),
            ..Default::default()
        },
        85.0,
    );
    let (h, v) = errors(&rx, &g);
    assert!(h < 12.0 && v < 20.0);
    let f = rx.fix.as_ref().unwrap();
    assert!(f.type_text.contains("Galileo"), "{}", f.type_text);
    let off = f.galileo_offset_ns.expect("the GST-GPS offset applied");
    assert!((off - 25.0).abs() < 0.1, "{off}");
}

#[test]
#[ignore]
fn beidou_alone() {
    let (rx, g) = run(
        GenOptions {
            systems: System::Beidou.bit(),
            centre_hz: 1561.098e6,
            rate_hz: 5e6,
            ..Default::default()
        },
        CoreConfig {
            systems: System::Beidou.bit(),
            week_ref: 2400,
            ..Default::default()
        },
        75.0,
    );
    let (h, v) = errors(&rx, &g);
    assert!(h < 15.0 && v < 25.0);
    assert_eq!(rx.masks().1 & System::Gps.bit(), 0, "L1 not active");
}

#[test]
#[ignore]
fn glonass_alone() {
    let (rx, g) = run(
        GenOptions {
            systems: System::Glonass.bit(),
            centre_hz: 1602e6,
            rate_hz: 10e6,
            ..Default::default()
        },
        CoreConfig {
            systems: System::Glonass.bit(),
            week_ref: 2400,
            ..Default::default()
        },
        70.0,
    );
    let (h, v) = errors(&rx, &g);
    assert!(h < 15.0 && v < 25.0);
    let f = rx.fix.as_ref().unwrap();
    assert!((f.glo_tau_gps_ns.unwrap() - 20.0).abs() < 1.0);
}
