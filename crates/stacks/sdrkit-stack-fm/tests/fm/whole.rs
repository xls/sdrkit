//! The whole receiver against RDS and multipath (1 Msps, station finder off).

use super::common::*;
use sdrkit_stack_fm::GenParams;

const RATE: f64 = 1e6;

fn station() -> GenParams {
    GenParams {
        rate_hz: RATE,
        gain: 0.3,
        ..Default::default()
    }
}

fn params(cnr: f64) -> Sig {
    sig(station(), cnr)
}

fn never_wrong(run: &Run, ps: &str, rt: &str) {
    for (i, r) in run.reports.iter().enumerate() {
        assert!(
            r.station.is_empty() || r.station == ps,
            "report {i}: PS {:?}",
            r.station
        );
        let ok = r.text.len() <= rt.len()
            && rt
                .chars()
                .zip(r.text.chars())
                .all(|(a, b)| a == b || b == ' ');
        assert!(ok, "report {i}: RT {:?}", r.text);
    }
}

#[test]
fn sixteen_db_stereo_and_mono() {
    for stereo in [true, false] {
        let mut rx = receiver(RATE, 100e6, &[]);
        let p = sig(
            GenParams {
                stereo,
                ..station()
            },
            16.0,
        )
        .seed(3);
        let secs = 20.0;
        let run = run(&mut rx, p, secs, |_| {});
        let d = &run.last;
        let gps = d.groups as f64 / secs;
        assert!(gps >= 8.0, "stereo {stereo}: {gps:.1} groups/s");
        let (good, corrected, bad) = (d.blocks_good, d.blocks_corrected, d.blocks_bad);
        let usable = 100.0 * (good + corrected) as f64 / (good + corrected + bad) as f64;
        assert!(usable >= 90.0, "stereo {stereo}: {usable:.1} % usable");
        never_wrong(&run, "SDRKIT  ", "sdrkit FM test signal");
        assert_eq!(d.station, "SDRKIT  ");
        eprintln!("stereo {stereo}: {gps:.1} groups/s, {usable:.1} % usable, PS after {:?} s, RT complete after {:?} s", run.first(|d| !d.station.is_empty()), run.first(|d| d.text_complete));
    }
}

#[test]
fn pilot_and_rds_only() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let p = sig(
        GenParams {
            left_amp: 0.0,
            right_amp: 0.0,
            ..station()
        },
        18.0,
    );
    let secs = 12.0;
    let run = run(&mut rx, p, secs, |_| {});
    let d = &run.last;
    assert!(d.groups as f64 / secs >= 10.0, "{} groups", d.groups);
    assert!(
        !d.equaliser || d.snr_db < 22.0,
        "equaliser {} (multipath {:.1} %, SNR {:.1})",
        d.equaliser,
        d.multipath_pct,
        d.snr_db
    );
    assert!(
        run.reports[60..].iter().all(|r| r.multipath_pct < 3.5),
        "multipath detected on a clean signal"
    );
}

#[test]
fn clean() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let run = run(&mut rx, params(40.0), 6.0, |_| {});
    let d = &run.last;
    assert!(
        !d.equaliser && d.stereo && d.stereo_pct > 99.0,
        "eq {} stereo {} {}",
        d.equaliser,
        d.stereo,
        d.stereo_pct
    );
}

#[test]
fn echo_10_us_whole() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let mut echo = echo(10, -6.0, RATE);
    let run = run(&mut rx, params(40.0), 8.0, |x| echo.process(x));
    let d = &run.last;
    assert!(
        d.equaliser,
        "equaliser off (multipath {:.1} %)",
        d.multipath_pct
    );
    assert!(d.rds_ok > 95.0, "usable {}", d.rds_ok);
    assert_eq!(d.station, "SDRKIT  ");
    eprintln!(
        "echo: multipath {:.1} %, usable {:.1} %, SNR {:.1}",
        d.multipath_pct, d.rds_ok, d.snr_db
    );
}

#[test]
fn twelve_db_blends() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let run = run(&mut rx, params(12.0), 6.0, |_| {});
    assert!(
        run.last.stereo_pct < 40.0,
        "stereo {} %",
        run.last.stereo_pct
    );
}

#[test]
fn rds_extras() {
    let common = (1u16 << 10) | (10 << 5);
    let name = b"OTHER   ";
    let mjd: u32 = 61336;
    let (h, m) = (12u16, 34u16);
    let mut extra = vec![
        [0x3000 | common | 22, 0x0000, 0x4BD7], // 3A: RT+ in 11A
        [
            0xB000 | common | 0x08,
            (4 << 13) | (5 << 1),
            (1 << 11) | (9 << 5) | 16,
        ], // 11A: Artist 0+5, Title 9+16
        [
            0x4000 | common | (mjd >> 15) as u16,
            (((mjd & 0x7FFF) as u16) << 1) | (h >> 4),
            ((h & 0xF) << 12) | (m << 6) | 6,
        ], // 4A
        [
            0xA000 | common,
            u16::from_be_bytes(*b"RO"),
            u16::from_be_bytes(*b"CK"),
        ], // 10A segment 0
        [
            0xA000 | common | 1,
            u16::from_be_bytes(*b"PO"),
            u16::from_be_bytes(*b"P "),
        ], // 10A segment 1
        [0x1000 | common, 0x00E2, (23 << 11) | (14 << 6) | 5], // 1A
    ];
    for v in 0..4u16 {
        extra.push([
            0xE000 | common | v,
            u16::from_be_bytes([name[2 * v as usize], name[2 * v as usize + 1]]),
            0x4B01,
        ]);
    }
    let p = sig(
        GenParams {
            rt: "Artist - Title of the song".into(),
            extra_groups: extra,
            ..station()
        },
        30.0,
    );
    let mut rx = receiver(RATE, 100e6, &[]);
    let run = run(&mut rx, p, 30.0, |_| {});
    let d = &run.last;
    let tag = |k: u8| d.rtplus.iter().find(|t| t.0 == k).map(|t| t.2.as_str());
    assert_eq!(tag(4), Some("Artist"));
    assert_eq!(tag(1), Some("Title of the song"));
    assert!(d.rtplus.iter().any(|t| t.1 == "Artist"));
    assert!(d.rtplus_running);
    assert_eq!(d.ptyn, "ROCKPOP ");
    assert_eq!(d.ecc, Some(0xE2));
    assert_eq!(d.pin, "23 14:05");
    assert_eq!(
        (d.clock, d.clock_offset_s),
        (Some(1_792_758_840.0), Some(180.0 * 60.0))
    );
    assert_eq!(d.eon, [(0x4B01, "OTHER   ".to_string())]);
    assert_eq!(d.oda, [(0x4BD7, "11A".to_string())]);
    assert_eq!(d.text, "Artist - Title of the song");
}
