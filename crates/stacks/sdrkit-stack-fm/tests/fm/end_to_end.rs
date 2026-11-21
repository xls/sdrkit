//! The receiver end to end, and round trips through the impairments with the timing targets.

use super::common::*;
use sdrkit_stack_fm::GenParams;

fn separation_db(run: &Run, secs: f64) -> (f64, f64, f64, f64) {
    let (l, r) = run.tail(secs);
    let (l1, r3) = (amp(&l, 1000.0), amp(&r, 3000.0));
    let (lr, rl) = (
        20.0 * (l1 / amp(&r, 1000.0)).log10(),
        20.0 * (r3 / amp(&l, 3000.0)).log10(),
    );
    (l1, r3, lr, rl)
}

fn assert_clean_station(run: &Run) {
    let d = &run.last;
    assert_eq!(d.state, 2, "locked");
    assert!(d.stereo, "stereo");
    assert!((7.0..=11.0).contains(&d.pilot_pct), "pilot {}", d.pilot_pct);
    let (l1, r3, lr, rl) = separation_db(run, 3.0);
    assert!((0.30..=0.50).contains(&l1), "left {l1}");
    assert!((0.30..=0.50).contains(&r3), "right {r3}");
    assert!(lr > 30.0 && rl > 30.0, "separation {lr:.1} / {rl:.1} dB");
    assert_rds_good(d, run.reports.len() as f64 * 0.1);
}

#[test]
fn two_msps_clean() {
    let rate = 2e6;
    let mut rx = receiver(rate, 100e6, &[]);
    let run = run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: rate,
                gain: 0.3,
                ..Default::default()
            },
            45.0,
        ),
        10.0,
        |_| {},
    );
    assert_clean_station(&run);
    // Timing targets on a clean signal: the name in about 0.6 s, the whole text in about 2.7 s.
    let t_ps = run.first(|d| d.station == "SDRKIT  ").unwrap();
    let t_rt = run.first(|d| d.text_complete).unwrap();
    assert!(t_ps <= 1.0, "PS after {t_ps} s");
    assert!(t_rt <= 3.3, "RT after {t_rt} s");
    assert!(
        ((run.last.deviation_hz / 1e3) - 53.0).abs() < 4.0,
        "deviation {}",
        (run.last.deviation_hz / 1e3)
    );
    assert!(
        (run.last.level_dbfs + 10.46).abs() < 0.3,
        "level {}",
        run.last.level_dbfs
    );
    assert!(
        (run.last.snr_db - 59.0).abs() < 2.5,
        "SNR {} (C/N + 14)",
        run.last.snr_db
    );
}

#[test]
fn odd_rate_and_seven_khz_offset() {
    let rate = 2.048e6;
    let mut rx = receiver(rate, 100e6, &[]);
    let run = run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: rate,
                offset_hz: 7000.0,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        ),
        10.0,
        |_| {},
    );
    assert_clean_station(&run);
    assert!(
        (run.last.offset_hz - 7000.0).abs() < 500.0,
        "offset {}",
        run.last.offset_hz
    );
}

#[test]
fn six_msps() {
    let rate = 6e6;
    let mut rx = receiver(rate, 100e6, &[]);
    let run = run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: rate,
                gain: 0.3,
                ..Default::default()
            },
            40.0,
        ),
        8.0,
        |_| {},
    );
    assert_clean_station(&run);
}

#[test]
fn mono_station() {
    let rate = 2e6;
    let mut rx = receiver(rate, 100e6, &[]);
    let p = sig(
        GenParams {
            rate_hz: rate,
            stereo: false,
            rds: false,
            right_hz: 1000.0,
            gain: 0.3,
            ..Default::default()
        },
        45.0,
    );
    let run = run(&mut rx, p, 5.0, |_| {});
    let d = &run.last;
    assert!(!d.stereo);
    assert_eq!(d.state, 2);
    let (l, r) = run.tail(3.0);
    let (a, b) = (amp(&l, 1000.0), amp(&r, 1000.0));
    assert!(
        (0.30..=0.50).contains(&a) && (0.30..=0.50).contains(&b),
        "{a} {b}"
    );
    assert!((a - b).abs() < 0.02);
}

#[test]
fn the_snr_measure_falls_with_the_carrier_to_noise() {
    let rate = 2e6;
    let mut last = f64::MAX;
    for cnr in [45.0, 30.0, 20.0, 12.0] {
        let mut rx = receiver(rate, 100e6, &[]);
        let run = run(
            &mut rx,
            sig(
                GenParams {
                    rate_hz: rate,
                    gain: 0.3,
                    ..Default::default()
                },
                cnr,
            ),
            3.0,
            |_| {},
        );
        let snr = run.last.snr_db;
        assert!(snr < last - 3.0, "C/N {cnr}: SNR {snr} after {last}");
        // About C/N + 14 dB on the generator.
        assert!(
            (snr - (cnr + 13.5)).abs() < 3.0 || snr >= 59.0,
            "C/N {cnr}: SNR {snr}"
        );
        last = snr;
    }
}

#[test]
fn noise_only() {
    let rate = 2e6;
    let mut rx = receiver(rate, 100e6, &[]);
    let run = run(
        &mut rx,
        sig(
            GenParams {
                rate_hz: rate,
                gain: 0.3,
                ..Default::default()
            },
            -25.0,
        ),
        3.0,
        |_| {},
    );
    let d = &run.last;
    assert_eq!(d.state, 0);
    assert!(!d.stereo && !d.carrier);
    assert_eq!(d.station, "");
    assert_eq!(d.snr_db, 0.0);
}

/// A round trip through independent impairments: noise at several C/N, a carrier offset and a clock error.
#[test]
fn round_trip_with_noise_offset_and_clock_error() {
    let rate = 1e6;
    for (cnr, offset_hz, ppm, ps_by, rt_by) in [
        (30.0, 3000.0, 40.0, 1.2, 3.6),
        (22.0, -4000.0, -60.0, 1.6, 6.0),
        (18.0, 2500.0, 20.0, 2.0, 9.0),
    ] {
        let p = sig(
            GenParams {
                rate_hz: rate,
                gain: 0.3,
                ..Default::default()
            },
            150.0,
        );
        // The generator's carrier has power 0.09; the noise is added here, measured in 200 kHz.
        let mut noise = awgn(0.09, cnr, rate, 77);
        let mut shift = offset(rate, offset_hz);
        let mut clock = clock(ppm);
        let mut rx = receiver(rate, 100e6, &[]);
        let mut tmp = Vec::new();
        let run = run(&mut rx, p, 10.0, |x| {
            shift.process(x);
            tmp.clear();
            clock.process(x, &mut tmp);
            noise.process(&mut tmp);
            x.clear();
            x.extend_from_slice(&tmp);
        });
        let d = &run.last;
        assert_rds_contents(d);
        assert!(d.stereo, "C/N {cnr}: stereo");
        assert!(
            (d.offset_hz - offset_hz).abs() < 300.0,
            "C/N {cnr}: offset {} against {offset_hz}",
            d.offset_hz
        );
        let t_ps = run.first(|d| d.station == "SDRKIT  ").unwrap();
        let t_rt = run.first(|d| d.text_complete).unwrap();
        assert!(t_ps <= ps_by, "C/N {cnr}: PS after {t_ps} s");
        assert!(t_rt <= rt_by, "C/N {cnr}: RT after {t_rt} s");
        // No wrong name or text was ever shown.
        for r in &run.reports {
            assert!(
                r.station.is_empty() || r.station == "SDRKIT  ",
                "C/N {cnr}: PS {:?}",
                r.station
            );
            assert!(
                "sdrkit FM test signal"
                    .chars()
                    .zip(r.text.chars())
                    .all(|(a, b)| a == b || b == ' ')
                    && r.text.len() <= 21
            );
        }
        eprintln!(
            "C/N {cnr}: PS {t_ps:.1} s, RT {t_rt:.1} s, groups {}, usable {:.0} %, SNR {:.1}",
            d.groups, d.rds_ok, d.snr_db
        );
    }
}

#[test]
fn live_audio_and_the_demo_station() {
    let rate = 1e6;
    let p = sig(
        GenParams {
            rate_hz: rate,
            rds: false,
            live_audio: true,
            gain: 0.3,
            ..Default::default()
        },
        50.0,
    );
    let mut g = p.source().g;
    // 3 s of 48 kHz stereo: 2 kHz on the left only.
    let audio: Vec<f32> = (0..3 * 48_000)
        .flat_map(|i| {
            [
                (0.4 * (std::f64::consts::TAU * 2000.0 * i as f64 / 48e3).sin()) as f32,
                0.0,
            ]
        })
        .collect();
    g.push_audio(&audio);
    let mut rx = receiver(rate, 100e6, &[]);
    let live = run_with(&mut rx, rate, 2.5, |buf, n| {
        buf.resize(n, sdrkit_core::Complex32::default());
        sdrkit_core::Generator::generate(&mut g, buf);
    });
    let (l, r) = live.tail(1.0);
    let (a, b) = (amp(&l, 2000.0), amp(&r, 2000.0));
    assert!((a - 0.32).abs() < 0.05, "left {a}");
    assert!(20.0 * (a / b).log10() > 30.0, "separation {a} / {b}");
    // The demo station: the long text, a quieter carrier.
    let demo = GenParams::demo(rate, 0.0);
    assert_eq!((demo.offset_hz, demo.gain), (0.0, 0.3));
    let mut rx = receiver(rate, 100e6, &[]);
    let run = run(&mut rx, sig(demo, 150.0), 1.5, |_| {});
    assert_eq!(run.last.station, "SDRKIT  ");
    assert!(
        (run.last.level_dbfs + 10.46).abs() < 0.5,
        "level {}",
        run.last.level_dbfs
    );
}
