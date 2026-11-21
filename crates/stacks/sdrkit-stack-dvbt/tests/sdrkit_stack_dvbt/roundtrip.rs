//! Round trips: generator, impairments, receiver, for every variant (2K and 8K, each constellation, rate, guard,
//! bandwidth, hierarchy and stream), with noise, carrier offset, clock error and multipath.

use crate::common::{run, summary, Case};
use sdrkit_stack_dvbt::params::*;

fn tps(mode: Mode, guard: Guard, c: Constellation, r: CodeRate) -> TpsParams {
    TpsParams {
        mode,
        guard,
        constellation: c,
        rate_hp: r,
        rate_lp: r,
        ..Default::default()
    }
}

fn check(name: &str, c: &Case, one_in: usize) {
    let o = run(c);
    eprintln!("{}", summary(name, &o));
    assert!(o.good(one_in), "{}", summary(name, &o));
}

#[test]
fn r1_2k_qpsk_half() {
    check(
        "R1",
        &Case::new(
            Bandwidth::B8,
            tps(Mode::K2, Guard::G8, Constellation::Qpsk, CodeRate::R12),
        ),
        20,
    );
}

#[test]
fn r3_8k_16qam_two_thirds() {
    check(
        "R3",
        &Case::new(
            Bandwidth::B8,
            tps(Mode::K8, Guard::G8, Constellation::Qam16, CodeRate::R23),
        ),
        20,
    );
}

#[test]
fn r4_cfo_3k2() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K2, Guard::G8, Constellation::Qam16, CodeRate::R23),
    );
    c.snr_db = Some(28.0);
    c.cfo_hz = 3200.0;
    check("R4", &c, 20);
}

#[test]
fn r6_echo() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K2, Guard::G4, Constellation::Qam16, CodeRate::R23),
    );
    c.echo = Some((60, -8.0, 0.0));
    check("R6", &c, 20);
}

#[test]
fn r7_clock_20ppm() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K8, Guard::G8, Constellation::Qam64, CodeRate::R23),
    );
    c.snr_db = Some(32.0);
    c.clock_ppm = 20.0;
    check("R7", &c, 20);
}

#[test]
fn c2_two_carriers_off() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K8, Guard::G4, Constellation::Qam16, CodeRate::R23),
    );
    c.cfo_hz = 2.1 * Bandwidth::B8.natural_rate() / 8192.0;
    check("C2", &c, 20);
}

#[test]
fn c5_minus_four_carriers_2k() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K2, Guard::G16, Constellation::Qpsk, CodeRate::R12),
    );
    c.cfo_hz = -3.9 * Bandwidth::B8.natural_rate() / 2048.0;
    check("C5", &c, 20);
}

#[test]
fn w3_5mhz_8k() {
    let mut c = Case::new(
        Bandwidth::B5,
        tps(Mode::K8, Guard::G32, Constellation::Qam16, CodeRate::R12),
    );
    c.snr_db = Some(32.0);
    c.cfo_hz = 43e3;
    check("W3", &c, 50);
}

#[test]
fn w4_6mhz_2k_received_wider() {
    let mut c = Case::new(
        Bandwidth::B6,
        tps(Mode::K2, Guard::G4, Constellation::Qpsk, CodeRate::R23),
    );
    c.snr_db = Some(32.0);
    c.cfo_hz = -43e3;
    check("W4", &c, 50);
}

#[test]
fn w2_10msps() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K8, Guard::G8, Constellation::Qam16, CodeRate::R23),
    );
    c.snr_db = Some(32.0);
    c.cfo_hz = -43e3;
    c.input_rate = Some(10e6);
    check("W2", &c, 50);
}

#[test]
fn w8_hierarchical_hp_and_w11_lp() {
    let mut t = tps(Mode::K8, Guard::G4, Constellation::Qam16, CodeRate::R12);
    t.hierarchy = Hierarchy::A1;
    t.rate_lp = CodeRate::R34;
    let mut c = Case::new(Bandwidth::B8, t);
    c.snr_db = Some(32.0);
    check("W8", &c, 50);
    let mut t = tps(Mode::K8, Guard::G4, Constellation::Qam16, CodeRate::R12);
    t.hierarchy = Hierarchy::A2;
    t.rate_lp = CodeRate::R12;
    let mut c = Case::new(Bandwidth::B8, t);
    c.snr_db = Some(32.0);
    c.lp = true;
    check("W11", &c, 50);
}

#[test]
fn w9_hierarchical_64qam_alpha_2() {
    let mut t = tps(Mode::K2, Guard::G8, Constellation::Qam64, CodeRate::R23);
    t.hierarchy = Hierarchy::A2;
    t.rate_lp = CodeRate::R56;
    let mut c = Case::new(Bandwidth::B8, t);
    c.snr_db = Some(32.0);
    check("W9", &c, 50);
    c.lp = true;
    c.tps.rate_lp = CodeRate::R23;
    check("W12-like LP", &c, 50);
}

#[test]
fn w21_iq_swapped() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K2, Guard::G4, Constellation::Qpsk, CodeRate::R12),
    );
    c.snr_db = Some(32.0);
    c.swap_iq = true;
    let o = run(&c);
    eprintln!("{}", summary("W22", &o));
    assert!(o.good(50));
    assert_eq!(
        o.results
            .fields()
            .at(&sdrkit_stack_dvbt::typed::DvbtFields::SPECTRUM_INVERTED),
        Some(true)
    );
}

#[test]
fn w23_nan_samples() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K8, Guard::G4, Constellation::Qam16, CodeRate::R23),
    );
    c.snr_db = Some(32.0);
    c.frames = 10;
    c.nan = Some((64, 1.0 / 3.0));
    check("W23", &c, 50);
}

#[test]
fn r18_dc_offset() {
    let mut c = Case::new(
        Bandwidth::B8,
        tps(Mode::K8, Guard::G16, Constellation::Qam64, CodeRate::R34),
    );
    c.cfo_hz = -155.0;
    c.dc = Some((-23.0, 0.7));
    check("R18", &c, 20);
}

#[test]
fn w15_off_centre_in_a_wide_recording() {
    let mut c = Case::new(
        Bandwidth::B7,
        tps(Mode::K2, Guard::G4, Constellation::Qpsk, CodeRate::R12),
    );
    c.snr_db = Some(32.0);
    c.input_rate = Some(10e6);
    c.cfo_hz = 1.3e6;
    let o = run(&c);
    eprintln!("{}", summary("W17", &o));
    assert!(o.good(50));
    assert!(
        (o.offset_hz - 1.3e6).abs() < 100.0,
        "offset {}",
        o.offset_hz
    );
}

#[test]
fn every_rate_and_guard_in_2k() {
    for (i, r) in CodeRate::ALL.into_iter().enumerate() {
        let g = Guard::ALL[i % 4];
        let mut c = Case::new(Bandwidth::B7, tps(Mode::K2, g, Constellation::Qam16, r));
        c.frames = 8;
        c.snr_db = Some(32.0);
        c.cfo_hz = 1234.0;
        c.clock_ppm = 10.0;
        check(&format!("2K 16-QAM {} {}", r.text(), g.text()), &c, 20);
    }
}
