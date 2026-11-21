//! Line-code guesses.

use crate::common::train;
use sdrkit_proto_ookdev::guess::{bits_bin, bits_hex, frame_ranges, Guesser, LineCode};
use sdrkit_proto_ookdev::Modulation;

#[test]
fn guesses_each_code() {
    let mut g = Guesser::default();
    // PPM: one mark length, two spaces, and a long gap between copies.
    let mut p = Vec::new();
    for c in 0..2 {
        for k in 0..20 {
            p.push((500.0, if k % 3 == 0 { 2000.0 } else { 1000.0 }));
        }
        if c == 0 {
            p.push((500.0, 9000.0));
        }
    }
    let r = g.guess(&train(Modulation::Ook, 1e6, &p));
    assert_eq!(r.code, LineCode::Ppm);
    assert!((r.short_us - 1000.0).abs() < 1.0 && (r.long_us - 2000.0).abs() < 1.0);
    assert_eq!(r.gap_us, 9000.0);

    // Manchester: half-bit 250 µs runs of one and two half-bits.
    let p: Vec<(f32, f32)> = (0..30)
        .map(|k| {
            if k % 4 == 0 {
                (500.0, 250.0)
            } else {
                (250.0, if k % 3 == 0 { 500.0 } else { 250.0 })
            }
        })
        .collect();
    let r = g.guess(&train(Modulation::Ook, 1e6, &p));
    assert_eq!(r.code, LineCode::Manchester);
    assert!((r.short_us - 250.0).abs() < 1.0);

    // NRZ: runs of 1, 3 and 5 bit widths of 100 µs.
    let p: Vec<(f32, f32)> = (0..30)
        .map(|k| ([100.0, 300.0, 500.0][k % 3], [300.0, 100.0, 500.0][k % 3]))
        .collect();
    let r = g.guess(&train(Modulation::Fsk, 1e6, &p));
    assert_eq!(r.code, LineCode::Nrz);
    assert!((r.short_us - 100.0).abs() < 1.0);

    // Nothing regular.
    let p: Vec<(f32, f32)> = (0..30)
        .map(|k| {
            (
                37.0 * (k as f32 + 1.0).powf(1.7),
                53.0 * (k as f32 + 2.0).powf(1.3),
            )
        })
        .collect();
    assert_eq!(
        g.guess(&train(Modulation::Ook, 1e6, &p)).code,
        LineCode::Unknown
    );
}

/// The bits of `s` ('0' and '1') as values.
fn bits_of(s: &str) -> Vec<u8> {
    s.bytes().map(|c| c - b'0').collect()
}

/// A PT2262-style frame: a 0 is 1T on and 3T off, a 1 is 3T on and 1T off, then a sync pulse of 1T on and 31T off.
fn pwm_frame(bits: &str, t: f32) -> Vec<(f32, f32)> {
    let mut p: Vec<(f32, f32)> = bits
        .bytes()
        .map(|c| {
            if c == b'1' {
                (3.0 * t, t)
            } else {
                (t, 3.0 * t)
            }
        })
        .collect();
    p.push((t, 31.0 * t));
    p
}

/// A PPM frame: a pulse, then a gap per bit, then a closing pulse; the last gap is `end`.
fn ppm_frame(bits: &str, pulse: f32, gap0: f32, gap1: f32, end: f32) -> Vec<(f32, f32)> {
    let mut p: Vec<(f32, f32)> = bits
        .bytes()
        .map(|c| (pulse, if c == b'1' { gap1 } else { gap0 }))
        .collect();
    p.push((pulse, end));
    p
}

/// A Manchester frame (1 is on-off, 0 is off-on) at half-bit `h`, as pulses from the first on half-bit; the last gap is `end`.
fn manchester_frame(bits: &str, h: f32, end: f32) -> Vec<(f32, f32)> {
    let halves: Vec<u8> = bits
        .bytes()
        .flat_map(|c| if c == b'1' { [1, 0] } else { [0, 1] })
        .collect();
    let mut p = Vec::new();
    let mut i = halves.iter().position(|&x| x == 1).unwrap();
    while i < halves.len() {
        let on = halves[i..].iter().take_while(|&&x| x == 1).count();
        i += on;
        let off = halves[i..].iter().take_while(|&&x| x == 0).count();
        i += off;
        p.push((
            on as f32 * h,
            if i >= halves.len() {
                end
            } else {
                off as f32 * h
            },
        ));
    }
    p
}

#[test]
fn reads_the_bits_of_each_test_pattern() {
    let mut g = Guesser::default();
    let mut bits = Vec::new();
    // PWM, T = 350 µs, the sync pulse dropped.
    let sent = "101100101001110010100110";
    let f = g.frame(&pwm_frame(sent, 350.0), &mut bits).unwrap();
    assert_eq!(f.code, LineCode::Pwm);
    assert_eq!(bits, bits_of(sent));
    assert_eq!((f.short_us, f.long_us), (Some(350.0), Some(1050.0)));
    assert_eq!(f.period_us, Some(1400.0));
    assert_eq!(f.pulses, 25);
    // The same frame at the end of a transmission.
    let mut p = pwm_frame(sent, 350.0);
    p.last_mut().unwrap().1 = f32::INFINITY;
    g.frame(&p, &mut bits).unwrap();
    assert_eq!(bits, bits_of(sent));

    // PPM: pulse 300 µs, gaps 500 and 1000 µs.
    let sent = "1100101001101011";
    let f = g
        .frame(&ppm_frame(sent, 300.0, 500.0, 1000.0, 20e3), &mut bits)
        .unwrap();
    assert_eq!(f.code, LineCode::Ppm);
    assert_eq!(bits, bits_of(sent));
    assert_eq!(
        (f.short_us, f.long_us, f.pulse_us),
        (Some(500.0), Some(1000.0), Some(300.0))
    );

    // Manchester, half-bit 250 µs: the last half-bit of a final 1 is lost in the gap after the frame.
    let sent = "10110010111000101101";
    let f = g
        .frame(&manchester_frame(sent, 250.0, 20e3), &mut bits)
        .unwrap();
    assert_eq!(f.code, LineCode::Manchester);
    assert_eq!(bits, bits_of(&sent[..19]));
    assert_eq!(f.unit_us, Some(250.0));
    assert_eq!(f.bit_rate(), Some(2000.0));
}

#[test]
fn reads_nrz_runs_and_refuses_noise_and_short_frames() {
    let mut g = Guesser::default();
    let mut bits = Vec::new();
    // NRZ-like runs of 1 to 3 units of 100 µs: 1 0 11 000 111 00 1 0 11.
    let p = [
        (100.0, 100.0),
        (200.0, 300.0),
        (300.0, 200.0),
        (100.0, 100.0),
        (200.0, f32::INFINITY),
    ];
    let f = g.frame(&p, &mut bits).unwrap();
    assert_eq!(f.code, LineCode::Nrz);
    assert_eq!(bits, bits_of("1011000111001011"));
    assert_eq!(f.bit_rate(), Some(10_000.0));
    // Widths that spread over more than 1.4 times within a cluster are noise.
    let p: Vec<(f32, f32)> = (0..20).map(|k| (300.0 + 15.0 * k as f32, 700.0)).collect();
    assert_eq!(g.frame(&p, &mut bits), None);
    // Fewer than 6 bits.
    assert_eq!(
        g.frame(&ppm_frame("1101", 300.0, 500.0, 1000.0, 20e3), &mut bits),
        None
    );
}

#[test]
fn frames_end_at_the_timeout() {
    // Two PWM frames back to back: the 10.85 ms sync gap is over 12 x the median width (4.2 ms) and ends each frame.
    let mut p = pwm_frame("101100101001110010100110", 350.0);
    p.extend(pwm_frame("101100101001110010100110", 350.0));
    let t = train(Modulation::Ook, 1e6, &p);
    let (mut scratch, mut out) = (Vec::new(), Vec::new());
    frame_ranges(&t, 5000.0, &mut scratch, &mut out);
    assert_eq!(out, vec![0..25, 25..50]);
    // A larger limit does not join them: the timeout is at most 12 x the median width.
    frame_ranges(&t, 50_000.0, &mut scratch, &mut out);
    assert_eq!(out, vec![0..25, 25..50]);
    // A limit under 1.2 ms cuts at every long gap.
    frame_ranges(&t, 1000.0, &mut scratch, &mut out);
    assert!(out.len() > 2, "{out:?}");
}

#[test]
fn the_bits_as_text() {
    let b = bits_of("101100101");
    assert_eq!(bits_bin(&b, 96), "101100101");
    assert_eq!(bits_bin(&b, 4), "1011...");
    assert_eq!(bits_hex(&b, 96), "B28");
}
