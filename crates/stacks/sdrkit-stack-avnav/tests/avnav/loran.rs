//! Loran chains from the chain data file: the names, and every GRI of the file found by the search among all of them.

use crate::common::get;
use sdrkit_core::{Complex32, Generator, InputInfo, Output};
use sdrkit_dev_test::Harness;
use sdrkit_stack_avnav::loran::{self, parse_chains, LoranGenerator, TestStation};
use sdrkit_stack_avnav::typed::{LoranFields, LORAN_STATIONS};
use sdrkit_stack_avnav::LORAN;

/// The chain data file.
pub const CHAIN_FILE: &str = include_str!("../../../../../data/navigation/loran_chains.csv");

#[test]
fn chain_names_come_from_the_file() {
    let chains = parse_chains(CHAIN_FILE);
    let name = |g: u32| chains.iter().find(|c| c.0 == g).map(|c| c.1.as_str());
    for (g, n) in [
        (9960, "Northeast U.S."),
        (8970, "Great Lakes"),
        (9940, "U.S. West Coast"),
        (5930, "Canadian East Coast"),
        (8000, "Western Russia Chayka"),
        (7950, "Eastern Russia Chayka"),
        (4970, "North-Western Russia Chayka"),
        (5970, "South Ural Chayka"),
    ] {
        assert_eq!(name(g), Some(n));
    }
    assert_eq!(name(1234), None);
    assert_eq!(chains.len(), 32);
    assert_eq!(
        parse_chains("9960,A;bad,B;# 7980,C\n7980, D "),
        [(9960, "A".to_string()), (7980, "D".to_string())]
    );
}

/// Each GRI of the table (6000 is an extra row of the file): 3.5 s of a master and two secondaries at 14 dB against the noise
/// in 20 kHz, off the centre by 25 or 30 kHz and a tuning error, found among all the GRIs of the file.
#[test]
fn every_chain_is_found_and_named() {
    let chains = parse_chains(CHAIN_FILE);
    let mut rng = sdrkit_algo_sim::rng::Rng::new(5);
    let mut wrong = Vec::new();
    for (i, (gri, name)) in chains.iter().filter(|c| c.0 != 6000).enumerate() {
        let rate = [250e3, 400e3, 500e3][i % 3];
        let off = [25e3, 30e3][i % 2] + rng.uniform() * 230.0;
        let max = f64::from(*gri) * 10.0 - 20000.0;
        let (d1, d2) = loop {
            let a = 8000.0 + rng.uniform() * (max - 8000.0);
            let b = 8000.0 + rng.uniform() * (max - 8000.0);
            if (a - b).abs() >= 4000.0 {
                break (a, b);
            }
        };
        let st = |master, td_us, amplitude| TestStation {
            master,
            td_us,
            ecd_us: 0.0,
            amplitude,
        };
        let stations = vec![st(true, 0.0, 1.0), st(false, d1, 0.9), st(false, d2, 0.7)];
        let mut g = LoranGenerator::with_stations(rate, *gri, stations, i as u64 + 1);
        let mut x = vec![Complex32::default(); (3.5 * rate) as usize];
        g.generate(&mut x);
        let w = std::f64::consts::TAU * off / rate;
        for (n, v) in x.iter_mut().enumerate() {
            *v *=
                Complex32::from_polar(1.0, (w * n as f64).rem_euclid(std::f64::consts::TAU) as f32);
        }
        // Master peak power 1, 14 dB above the noise in 20 kHz.
        let noise = 10f64.powf(-1.4) * rate / 20e3;
        sdrkit_algo_sim::noise::Awgn::with_power(noise, i as u64 + 100)
            .unwrap()
            .process(&mut x);
        let mut rx = Harness::new(&LORAN)
            .set(&loran::CHAINS, CHAIN_FILE)
            .receiver()
            .unwrap();
        rx.configure(&InputInfo::new(rate, 100e3 - off, 100e3).recording())
            .unwrap();
        let mut out = Output::default();
        for c in x.chunks(50_000) {
            rx.feed(c, &mut out);
            out.clear();
        }
        let r = rx.results();
        let found = get(r, &LoranFields::GRI);
        let rows = r
            .decl()
            .table_id(LORAN_STATIONS.key)
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.rows_newest_first().count());
        let ok = found == Some(i64::from(*gri))
            && get(r, &LoranFields::MASTER) == Some(true)
            && rows == 3
            && get(r, &LoranFields::CHAIN) == Some(name.as_str());
        if !ok {
            wrong.push(format!(
                "{gri}: GRI {found:?}, {rows} stations, chain {:?}",
                get(r, &LoranFields::CHAIN)
            ));
        }
    }
    assert!(wrong.is_empty(), "{wrong:?}");
}
