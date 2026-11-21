//! Tests of faults that were found and fixed: the RDS decoder, the band scan and the band plans.

use super::common::*;
use sdrkit_core::SettingSet;

use sdrkit_stack_fm::rds::{Group, GroupDecoder};
use sdrkit_stack_fm::scan::{BandScan, ScanCommand};
use sdrkit_stack_fm::{FmSetting, GenParams, Reading};
use sdrkit_util_text::ebu::RdsCharset;

const PI: u16 = 0x4A21;

/// The RadioText of one 2A group carrying `bytes` in segment 0 (four bytes, then a carriage return in segment 1).
fn radiotext(bytes: [u8; 4]) -> String {
    let mut d = GroupDecoder::new(RdsCharset::EbuLatin);
    let b = 0x2000 | (10 << 5);
    let w = |a: u8, c: u8| u16::from_be_bytes([a, c]);
    assert!(d.push(
        &Group::clean(PI, b, w(bytes[0], bytes[1]), w(bytes[2], bytes[3])),
        0.0
    ));
    assert!(d.push(&Group::clean(PI, b | 1, w(0x0D, b' '), w(b' ', b' ')), 0.0));
    d.rt.clone()
}

/// IEC 62106 Annex E: 0x24, 0x5E, 0x60 and 0x7E are not the ASCII characters.
#[test]
fn the_rds_table_follows_annex_e_in_its_lower_half() {
    assert_eq!(radiotext([0x24, 0x5E, 0x60, 0x7E]), "¤―‖¯");
}

/// A line break in a RadioText is a line break, not a question mark.
#[test]
fn radiotext_control_codes_are_not_question_marks() {
    let rt = radiotext([b'A', 0x0A, b'B', 0x1F]);
    assert_eq!(rt, "A\nB\u{AD}");
    assert!(!rt.contains('?'));
}

/// A station heard in Australia on an odd tenth (104.1 MHz), with a PI in the RBDS K range and no country code: the host
/// chose RDS, so the RDS meanings apply and no call letters ("KGEJ") are shown.
#[test]
fn a_station_in_australia_reads_rds_on_an_odd_tenth() {
    let mut rx = receiver(2e6, 104.1e6, &[("rds_variant", "rds")]);
    let p = GenParams {
        rate_hz: 2e6,
        pi: 0x2049,
        gain: 0.3,
        ..Default::default()
    };
    let d = run(&mut rx, sig(p, 40.0), 2.0, |_| {}).last;
    assert_eq!(d.pi, 0x2049);
    assert_eq!(
        (d.rbds, d.programme.as_str(), d.call_letters.as_str()),
        (false, "Pop Music", "")
    );
}

/// A band scan on two adjacent channels of one station (the same PI 0.1 MHz apart) and a station with an incomplete text.
fn scan_two_channels(complete_second: bool) -> BandScan {
    let mut s = BandScan::new(104e6, 106e6, false, 100e6);
    s.start(0.0);
    let n = 2048;
    let spec: Vec<f32> = (0..n)
        .map(|i| {
            let f = 105e6 + (i as f64 - (n / 2) as f64) * 2e6 / n as f64;
            if (f - 104.7e6).abs() < 30e3 || (f - 104.8e6).abs() < 30e3 {
                -60.0
            } else {
                -100.0
            }
        })
        .collect();
    assert_eq!(
        s.on_spectrum(0.8, 105e6, 2e6, &spec),
        Some(ScanCommand::Check { hz: 104.7e6 })
    );
    let station = |snr: f64, complete: bool| Reading {
        carrier: true,
        state: 2,
        station: "TRIPLE M".into(),
        programme: "Rock".into(),
        text: "     FOR     GREATEST SONG EVER ON".into(),
        text_complete: complete,
        pi: 0x2049,
        snr_db: snr,
        ..Default::default()
    };
    assert_eq!(
        s.on_report(2.6, &station(25.0, false)),
        Some(ScanCommand::Check { hz: 104.8e6 })
    );
    assert_eq!(
        s.on_report(4.5, &station(12.0, complete_second)),
        Some(ScanCommand::Done { return_hz: 100e6 })
    );
    s
}

#[test]
fn the_scan_keeps_one_station_per_pi_on_adjacent_steps() {
    let s = scan_two_channels(true);
    assert_eq!(s.stations.len(), 1);
    assert_eq!(s.stations[0].mhz, 104.7);
}

#[test]
fn a_partial_radiotext_is_not_shown_as_final() {
    let s = scan_two_channels(true);
    assert!(!s.stations[0].text_complete);
    assert_eq!(s.stations[0].note, "FOR GREATEST SONG EVER ON (partial)");
}

/// The FM band of New Zealand in the data (87.5 to 108 MHz on 100 kHz steps) reaches the band settings.
#[test]
fn the_new_zealand_band_reaches_the_band_settings() {
    let nz = band_settings("nz");
    let schema = FmSetting::schema();
    for (key, text) in &nz {
        let (id, v) = value(key, text);
        assert!(schema.check(id, &v).is_ok(), "{key} {text}");
    }
    let mut rx = receiver(1e6, 99e6, &pairs(&[], &nz));
    run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: 1e6,
                ..Default::default()
            },
            150.0,
        ),
        0.2,
        |_| {},
    );
    let p = detail(&rx).band;
    assert_eq!((p.first_hz, p.step_hz), (87.5e6, 100e3));
}
