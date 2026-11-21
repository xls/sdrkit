//! The band scan with the real receiver, TMC and EON through the generator, band plans and channel filter presets.

use super::common::*;
use sdrkit_algo_dsp::spectrum::{SpectrumAnalyzer, SpectrumConfig};
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::{Complex32, InputInfo, Output, Receiver, Value};
use sdrkit_stack_fm::scan::{BandScan, ScanCommand};
use sdrkit_stack_fm::typed::*;
use sdrkit_stack_fm::GenParams;

const RATE: f64 = 2e6;

fn station(ps: &str, pi: u16, offset_hz: f64, seed: u64) -> Noisy {
    sig(
        GenParams {
            rate_hz: RATE,
            ps: ps.into(),
            pi,
            offset_hz,
            rt: format!("{} text", ps.trim()),
            gain: 0.3,
            ..Default::default()
        },
        45.0,
    )
    .seed(seed)
    .source()
}

/// Two stations, at 88.5 and 89.3 MHz, as a radio tuned to `centre_hz` sees them.
fn band(centre_hz: f64, seed: u64) -> [Noisy; 2] {
    [
        station("ONE", 0x1111, 88.5e6 - centre_hz, seed),
        station("TWO", 0x2222, 89.3e6 - centre_hz, seed + 7),
    ]
}

fn mix(gens: &mut [Noisy; 2], n: usize) -> Vec<Complex32> {
    let mut a = vec![Complex32::default(); n];
    let mut b = vec![Complex32::default(); n];
    gens[0].generate(&mut a);
    gens[1].generate(&mut b);
    sdrkit_algo_sim::front::add_into(&mut a, &b);
    a
}

#[test]
fn the_band_scan_finds_the_stations_and_their_names() {
    let mut rx = receiver(RATE, 95e6, &[("scan_low", "88"), ("scan_high", "90")]);
    let mut scan = BandScan::new(88e6, 90e6, false, 95e6);
    let mut now = 0.0;
    let mut cmd = scan.start(now);
    let mut out = Output::default();
    let mut guard = 0;
    loop {
        guard += 1;
        assert!(guard < 1000, "the scan does not end");
        match cmd {
            ScanCommand::Survey { centre_hz } => {
                // The engine's spectrum at this centre.
                let mut gens = band(centre_hz, 1);
                let mut cfg = SpectrumConfig::new(2048, RATE);
                cfg.average = 8;
                let mut sa = SpectrumAnalyzer::new(cfg).unwrap();
                sa.push(&mix(&mut gens, (0.8 * RATE) as usize));
                now += 0.8;
                cmd = scan
                    .on_spectrum(now, centre_hz, RATE, sa.spectrum_db())
                    .expect("a frame after 0.7 s ends the hop");
            }
            ScanCommand::Check { hz } => {
                rx.retuned(&InputInfo {
                    rate_hz: RATE,
                    centre_hz: hz,
                    tuned_hz: hz,
                    live: true,
                });
                let mut gens = band(hz, 3);
                cmd = loop {
                    out.clear();
                    rx.feed(&mix(&mut gens, (RATE / 10.0) as usize), &mut out);
                    now += 0.1;
                    if let Some(c) = scan.on_report(now, &detail(&rx)) {
                        break c;
                    }
                };
            }
            ScanCommand::Done { return_hz } => {
                assert_eq!(return_hz, 95e6);
                break;
            }
        }
    }
    let names: Vec<(f64, &str, &str)> = scan
        .stations
        .iter()
        .map(|s| (s.mhz, s.name.as_str(), s.note.as_str()))
        .collect();
    assert_eq!(
        names,
        vec![(88.5, "ONE", "ONE text"), (89.3, "TWO", "TWO text")],
        "{:?}",
        scan.candidates()
    );
    assert_eq!(scan.stations[0].pi, 0x1111);
    assert!(scan.stations.iter().all(|s| s.rds && s.stereo && s.tp));
    assert_eq!(scan.stations[1].mhz, 89.3);
}

#[test]
fn tmc_and_eon_through_the_generator() {
    let common = (1u16 << 10) | (10 << 5);
    let extra = vec![
        [0x3000 | common | 16, (12 << 6) | 0x20, 0xCD46], // 3A: TMC in 8A, LTN 12, AFI
        [0x8000 | common | 0x08 | 3, 0x4000 | (2 << 11) | 101, 12345], // 8A single group
        [0xE000 | common | 0x10 | 13, (5 << 11) | 1, 0x4B01], // 14A variant 13: PTY 5, TA, TP of the other network
        [0xE000 | common | 4, (45 << 8) | 80, 0x4B01],        // 14A variant 4: AF 92.0 and 95.5 MHz
    ];
    let p = sig(
        GenParams {
            rate_hz: 1e6,
            extra_groups: extra,
            gain: 0.3,
            ..Default::default()
        },
        35.0,
    );
    let mut rx = receiver(1e6, 100e6, &[]);
    let run = run(&mut rx, p, 6.0, |_| {});
    let r = Owner::results(&rx);
    assert_eq!(r.fields().at(&FmFields::TMC_LTN), Some(12));
    assert_eq!(r.fields().at(&FmFields::TMC_AFI), Some(true));
    let table = |t: &sdrkit_core::TableDef| r.table(r.decl().table_id(t.key).unwrap()).unwrap();
    let tmc = table(&TMC);
    assert_eq!(tmc.len(), 1);
    let m = tmc.row("00").unwrap();
    assert_eq!(
        (
            m.at(&TmcRow::EVENT),
            m.at(&TmcRow::LOCATION),
            m.at(&TmcRow::NEGATIVE_DIRECTION),
            m.at(&TmcRow::EXTENT),
            m.at(&TmcRow::DURATION)
        ),
        (Some(101), Some(12345), Some(true), Some(2), Some(3))
    );
    assert!(m.at(&TmcRow::REPEATS).is_some_and(|n| n >= 2));
    let d = &run.last;
    assert_eq!(d.oda, [(0xCD46, "8A".to_string())]);
    assert_eq!(d.eon.len(), 1);
    let e = table(&EON).row("4B01").unwrap();
    assert_eq!(
        (
            e.at(&EonRow::PI),
            e.at(&EonRow::TP),
            e.at(&EonRow::TA),
            e.at(&EonRow::PTY)
        ),
        (Some("4B01"), Some(true), Some(true), Some(5))
    );
    let af = table(&EON_AF);
    let mut freqs: Vec<f64> = af
        .rows_newest_first()
        .filter(|(_, row)| row.at(&EonAfRow::NETWORK) == Some("4B01"))
        .filter_map(|(_, row)| row.at(&EonAfRow::FREQ_HZ))
        .collect();
    freqs.sort_by(f64::total_cmp);
    assert_eq!(freqs, [92.0e6, 95.5e6]);
}

#[test]
fn bands_of_two_rows_of_the_data_reach_the_results() {
    let us = band_settings("north_america");
    let mut rx = receiver(1e6, 101.1e6, &pairs(&[("channel_filter", "wide")], &us));
    let p = || {
        sig(
            GenParams {
                rate_hz: 1e6,
                gain: 0.3,
                ..Default::default()
            },
            150.0,
        )
    };
    let first = run(&mut rx, p(), 0.5, |_| {});
    let r = Owner::results(&rx);
    assert_eq!(first.last.band.max_hz, 108e6);
    assert_eq!(
        (
            r.fields().at(&FmFields::BAND_STEP_HZ),
            r.fields().at(&FmFields::BAND_FIRST_HZ)
        ),
        (Some(200e3), Some(88.1e6))
    );
    assert_eq!(first.last.channel_width_hz, 300e3);
    let mut rx = receiver(1e6, 70e6, &pairs(&[], &band_settings("oirt")));
    for (key, text) in band_settings("japan") {
        set(&mut rx, &key, &text).unwrap();
    }
    let low = rx.schema().id("band_low_hz").unwrap();
    assert!(owner::apply(&mut rx, low, &Value::Choice(u16::MAX)).is_err());
    run(&mut rx, p(), 0.2, |_| {});
    let r = Owner::results(&rx);
    assert_eq!(
        (
            r.fields().at(&FmFields::BAND_START_HZ),
            r.fields().at(&FmFields::BAND_END_HZ)
        ),
        (Some(76e6), Some(98.95e6))
    );
}
