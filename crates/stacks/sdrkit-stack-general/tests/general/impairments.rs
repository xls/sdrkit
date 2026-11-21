//! Round trips: the generator, then independent impairments (noise, frequency offset, clock error, multipath), then the receiver,
//! for every demodulator.

use crate::common::*;

const RATE: f64 = 2e6;

/// The test signal through the framework's impairments: a carrier offset, a clock error, an echo of 3 samples at -12 dB (1 rad)
/// and noise in 12.5 kHz.
fn impaired(demod: &str, offset_hz: f64, ppm: f64, echo: bool, snr: f64) -> Source {
    let (off, clock, snr) = (offset_hz.to_string(), ppm.to_string(), snr.to_string());
    let mut s = vec![
        ("gen_offset_hz", off.as_str()),
        ("gen_clock_ppm", clock.as_str()),
        ("gen_snr_db", snr.as_str()),
    ];
    if echo {
        s.extend_from_slice(&[
            ("gen_fading", "echo"),
            ("gen_echo_delay_s", "1.5e-6"),
            ("gen_echo_db", "-12"),
            ("gen_echo_phase_deg", "57.3"),
        ]);
    }
    generator(RATE, demod, &s)
}

fn round_trip(demod: &str, offset_hz: f64, ppm: f64, echo: bool, snr: f64) -> (f64, f64, Run) {
    let mut g = impaired(demod, offset_hz, ppm, echo, snr);
    let mut rx = receiver(RATE, -250e3, &[("demod", demod), ("deemph_wfm", "off")]);
    let r = run_gen(&mut rx, &mut g, 1.5, 20_000);
    let f = match demod {
        "cw" => 700.0 + offset_hz,
        "usb" => 1000.0 + offset_hz,
        "lsb" => 1000.0 - offset_hz,
        _ => 1000.0,
    };
    // The clock error moves the station a few hertz too (20 ppm of 250 kHz is 5 Hz): take the tone's peak near its frequency.
    let t = r.tail(0.5);
    let (a, fa) = (-40..=40)
        .map(|k| f + k as f64 * 0.25)
        .map(|x| (tone(t, x), x))
        .fold((0.0, f), |m, v| if v.0 > m.0 { v } else { m });
    (a, fa, r)
}

fn db(x: f64) -> f64 {
    20.0 * x.max(1e-12).log10()
}

#[test]
fn every_demodulator_survives_the_impairments() {
    for (d, want) in [
        ("nfm", 0.25),
        ("wfm", 0.25),
        ("am", 0.25),
        ("usb", 0.5),
        ("lsb", 0.5),
        ("cw", 0.5),
    ] {
        // A carrier off frequency, a clock 20 ppm fast, an echo, and noise at 25 dB in 12.5 kHz.
        let off = if d == "wfm" || d == "nfm" {
            800.0
        } else {
            40.0
        };
        let (a, f, r) = round_trip(d, off, 20.0, true, 25.0);
        assert!((db(a) - db(want)).abs() < 1.5, "{d}: {a} at {f} Hz");
        let l = r.last();
        // WFM measures its noise over 180 kHz: 25 dB in 12.5 kHz is about 13 dB there.
        let min_snr = if d == "wfm" { 9.0 } else { 15.0 };
        assert!(l.squelch_open && l.snr_db > min_snr, "{d}: {}", l.summary);
        if d == "nfm" || d == "wfm" {
            assert!(
                (l.freq_offset_hz - off).abs() < 60.0,
                "{d}: the FM offset reads {}",
                l.freq_offset_hz
            );
        }
    }
}

#[test]
fn heavy_noise_still_opens_and_decodes_nfm() {
    let (a, _, r) = round_trip("nfm", 0.0, 0.0, false, 12.0);
    assert!(r.last().squelch_open);
    assert!(a > 0.15, "{a}");
}

#[test]
fn synchronous_am_through_the_impairments() {
    let mut g = generator(
        RATE,
        "am",
        &[
            ("gen_offset_hz", "120"),
            ("gen_clock_ppm", "-30"),
            ("gen_fading", "echo"),
            ("gen_echo_delay_s", "2.5e-6"),
            ("gen_echo_db", "-10"),
            ("gen_echo_phase_deg", "114.6"),
            ("gen_snr_db", "30"),
        ],
    );
    let mut rx = receiver(
        RATE,
        -250e3,
        &[
            ("demod", "am"),
            ("am_auto", "0"),
            ("am_detector", "synchronous"),
        ],
    );
    let r = run_gen(&mut rx, &mut g, 3.0, 20_000);
    let a = tone(r.tail(1.0), 1000.0);
    assert!((a - 0.25).abs() < 0.04, "{a}");
    let am = &r.last().am;
    assert!(am.active && am.locked, "{}", am.line);
    // The clock 30 ppm slow moves the station (250 kHz from the centre) by 7.5 Hz as well.
    let want = 120.0 - 250e3 * 30e-6;
    assert!(
        (am.carrier_offset_hz - want).abs() < 2.0,
        "{}",
        am.carrier_offset_hz
    );
}
