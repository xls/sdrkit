//! Impairments added independently of the generator (2 Msps, generator defaults).

use super::common::*;
use sdrkit_algo_sim::front::add_into;
use sdrkit_core::Complex32;
use sdrkit_stack_fm::GenParams;

const RATE: f64 = 2e6;

/// The generator's station at a carrier amplitude of 0.3.
fn station() -> GenParams {
    GenParams {
        rate_hz: RATE,
        gain: 0.3,
        ..Default::default()
    }
}

/// The station at 40 dB C/N.
fn params() -> Sig {
    sig(station(), 40.0)
}

/// A changed station at 40 dB C/N.
fn with(p: GenParams) -> Sig {
    sig(p, 40.0)
}

#[test]
fn bad_samples() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let mut n = 0usize;
    let secs = 6.0;
    let run = run(&mut rx, params(), secs, |x| {
        let at = (0.5 * RATE) as usize;
        if (n..n + x.len()).contains(&at) {
            x[at - n] = Complex32::new(f32::NAN, 0.0);
            x[at - n + 1] = Complex32::new(f32::INFINITY, -f32::INFINITY);
        }
        n += x.len();
    });
    let d = &run.last;
    assert!(
        d.offset_hz.is_finite() && d.offset_hz.abs() < 200.0,
        "offset {}",
        d.offset_hz
    );
    assert!(
        d.stereo && d.pilot_pct > 7.0,
        "stereo {} pilot {}",
        d.stereo,
        d.pilot_pct
    );
    assert!(d.snr_db > 40.0 && d.level_dbfs.is_finite());
    assert_rds_good(d, secs);
}

fn clock_error(ppm: f64) {
    let mut rx = receiver(RATE, 100e6, &[]);
    let mut clock = clock(ppm);
    let mut tmp = Vec::new();
    let run = run(&mut rx, params(), 16.0, |x| {
        tmp.clear();
        clock.process(x, &mut tmp);
        x.clear();
        x.extend_from_slice(&tmp);
    });
    // Judged as for 8 s.
    assert_rds_good(&run.last, 8.0);
    assert!(run.last.stereo);
}

#[test]
fn clock_plus_100_ppm() {
    clock_error(100.0);
}

#[test]
fn clock_minus_100_ppm() {
    clock_error(-100.0);
}

#[test]
fn mono_station_with_rds() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let secs = 8.0;
    let run = run(
        &mut rx,
        with(GenParams {
            stereo: false,
            ..station()
        }),
        secs,
        |_| {},
    );
    assert!(!run.last.stereo && !run.last.pilot_locked);
    assert_rds_good(&run.last, secs);
}

#[test]
fn dc_spike_below_an_offset_station() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let secs = 6.0;
    let dc = Complex32::new(0.3 * 10f32.powf(-6.0 / 20.0), 0.0);
    let run = run(
        &mut rx,
        with(GenParams {
            offset_hz: 1200.0,
            ..station()
        }),
        secs,
        |x| x.iter_mut().for_each(|v| *v += dc),
    );
    let d = &run.last;
    assert_eq!(d.state, 2);
    assert!(d.stereo);
    assert!(
        (d.offset_hz - 1200.0).abs() < 200.0,
        "offset {}",
        d.offset_hz
    );
    assert_rds_good(d, secs);
}

/// The wanted station plus a second one, mixed at the input.
fn two_stations(rx: &mut Rx, wanted: Sig, other: Sig, secs: f64) -> Run {
    let mut g2 = other.source();
    let mut buf = Vec::new();
    run(rx, wanted, secs, move |x| {
        buf.resize(x.len(), Complex32::default());
        g2.generate(&mut buf);
        add_into(x, &buf);
    })
}

fn other_station(offset_hz: f64, gain_db: f64) -> Sig {
    sig(
        GenParams {
            offset_hz,
            gain: 0.3 * 10f64.powf(gain_db / 20.0),
            ps: "OTHER   ".into(),
            pi: 0x1234,
            rt: "Other station".into(),
            ..station()
        },
        150.0,
    )
}

fn finder_moves(wanted_hz: f64) {
    let mut rx = receiver(RATE, 100e6, &[("station_finder", "1")]);
    let secs = 6.0;
    let run = two_stations(
        &mut rx,
        with(GenParams {
            offset_hz: wanted_hz,
            ..station()
        }),
        other_station(
            wanted_hz + if wanted_hz > 0.0 { -650e3 } else { 650e3 },
            -15.0,
        ),
        secs,
    );
    let d = &run.last;
    assert_eq!(d.state, 2, "locked");
    assert!(d.stereo);
    assert!(
        (d.offset_hz - wanted_hz).abs() < 2000.0,
        "offset {} against {wanted_hz}",
        d.offset_hz
    );
    assert_rds_good(d, secs);
    for r in &run.reports {
        assert!(
            r.station.is_empty() || r.station == "SDRKIT  ",
            "{:?}",
            r.station
        );
    }
}

#[test]
fn finder_moves_to_a_station_above() {
    finder_moves(420e3);
}

#[test]
fn finder_moves_to_a_station_below() {
    finder_moves(-560e3);
}

#[test]
fn finder_keeps_a_weak_station_at_the_centre() {
    let mut rx = receiver(RATE, 100e6, &[("station_finder", "1")]);
    let secs = 6.0;
    let run = two_stations(
        &mut rx,
        with(GenParams {
            gain: 0.03,
            ..station()
        }),
        other_station(400e3, 0.0),
        secs,
    );
    let d = &run.last;
    assert!(d.offset_hz.abs() < 2000.0, "offset {}", d.offset_hz);
    assert_rds_good(d, secs);
}

#[test]
fn narrow_channel_keeps_out_a_strong_neighbour() {
    let secs = 6.0;
    // The narrow preset, and the same width set by hand.
    for settings in [
        &[("channel_filter", "custom"), ("channel_width", "150000")][..],
        &[("channel_filter", "narrow")][..],
    ] {
        let mut rx = receiver(RATE, 100e6, settings);
        let run = two_stations(&mut rx, params(), other_station(150e3, 10.0), 3.0);
        assert_eq!(run.last.channel_width_hz, 150e3);
        assert!(
            run.last.state == 2 && run.last.stereo && run.last.snr_db > 30.0,
            "{settings:?}: SNR {}",
            run.last.snr_db
        );
    }
    let mut rx = receiver(RATE, 100e6, &[("channel_width", "150000")]);
    let run = two_stations(&mut rx, params(), other_station(150e3, 10.0), secs);
    let d = &run.last;
    assert_eq!(d.state, 2, "locked (SNR {})", d.snr_db);
    assert!(d.stereo);
    assert!(d.snr_db > 30.0, "SNR {}", d.snr_db);
    // For comparison: the standard width is corrupted by the same neighbour.
    let mut rx = receiver(RATE, 100e6, &[]);
    let wide = two_stations(&mut rx, params(), other_station(150e3, 10.0), 2.0);
    eprintln!(
        "standard width: state {} SNR {:.1} PS {:?}; 150 kHz: SNR {:.1}, usable {:.0} %",
        wide.last.state, wide.last.snr_db, wide.last.station, d.snr_db, d.rds_ok
    );
}

#[test]
fn echo_10_us() {
    let mut rx = receiver(RATE, 100e6, &[]);
    let secs = 8.0;
    let mut echo = echo(20, -6.0, RATE);
    let run = run(&mut rx, params(), secs, |x| echo.process(x));
    let d = &run.last;
    assert_eq!(d.state, 2);
    assert!(d.stereo);
    assert_rds_good(d, secs);
}

fn combined(stereo: bool) {
    let mut rx = receiver(RATE, 100e6, &[]);
    let secs = 10.0;
    let mut shift = offset(RATE, 5400.0);
    let mut clock = clock(80.0);
    let mut echo = echo(6, -8.0, RATE);
    let mut tmp = Vec::new();
    let run = run(
        &mut rx,
        with(GenParams {
            stereo,
            ..station()
        }),
        secs,
        |x| {
            shift.process(x);
            tmp.clear();
            clock.process(x, &mut tmp);
            echo.process(&mut tmp);
            quantise(&mut tmp, 8, 0.4);
            x.clear();
            x.extend_from_slice(&tmp);
        },
    );
    let d = &run.last;
    assert_eq!(d.state, 2);
    assert!(
        (d.offset_hz - 5400.0).abs() < 300.0,
        "offset {}",
        d.offset_hz
    );
    assert_eq!(d.stereo, stereo);
    assert_rds_good(d, secs);
}

#[test]
fn combined_stereo() {
    combined(true);
}

#[test]
fn combined_mono() {
    combined(false);
}
