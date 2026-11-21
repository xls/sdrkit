//! NOAA APT: the line format and the receiver on its test signal.

use super::common::{run, Ran};
use sdrkit_algo_sim::chain::{
    NoiseKind, GEN_CLOCK_PPM, GEN_DRIFT_HZ_S, GEN_LEVEL_DBFS, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB,
};
use sdrkit_codec_image::strip::Strip;
use sdrkit_core::{Complex32, LockState};
use sdrkit_dev_test::Harness;
use sdrkit_stack_satimg::gen::test_value;
use sdrkit_stack_satimg::instruments::apt::{
    sync_a, sync_b, LINE_WORDS, PICTURE_A, PICTURE_B, PICTURE_WORDS, SYNC_B, SYNC_WORDS,
};
use sdrkit_stack_satimg::modes::apt::{CHANNEL_A, CHANNEL_B, DESCRIPTOR, FULL};

const RATE: f64 = 960e3;

/// The test signal of `secs` seconds with a carrier-to-noise ratio in 40 kHz, an offset, a drift and a clock error, through
/// the receiver.
fn go(secs: f64, cn_db: f64, offset_hz: f64, drift_hz_s: f64, ppm: f64) -> Ran {
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .seconds(secs)
        .set(&GEN_LEVEL_DBFS, -13.0)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, cn_db)
        .set(&GEN_OFFSET_HZ, offset_hz)
        .set(&GEN_DRIFT_HZ_S, drift_hz_s)
        .set(&GEN_CLOCK_PPM, ppm);
    let x = h.generate().expect("a signal");
    run::<Strip>(&h, &x, RATE, 137.1e6, 137.1e6, 65_536, Some("apt.out"))
}

#[test]
fn sync_a_is_seven_pulses_from_word_4() {
    let a = sync_a();
    let text: String = a.iter().map(|&h| if h { '1' } else { '0' }).collect();
    assert_eq!(text, "000011001100110011001100110011000000000");
    assert_eq!(a.iter().filter(|&&h| h).count(), 14);
    assert_eq!(a.iter().filter(|&&h| !h).count(), 25);
    let b = sync_b();
    let text: String = b.iter().map(|&h| if h { '1' } else { '0' }).collect();
    assert_eq!(text, "000011100111001110011100111001110011100");
}

#[test]
fn the_line_has_2080_words() {
    assert_eq!(39 + 47 + 909 + 45 + 39 + 47 + 909 + 45, LINE_WORDS);
    assert_eq!(SYNC_WORDS + 47, PICTURE_A);
    assert_eq!(PICTURE_A + PICTURE_WORDS + 45, SYNC_B);
    assert_eq!(SYNC_B + SYNC_WORDS + 47, PICTURE_B);
    assert_eq!(PICTURE_B + PICTURE_WORDS + 45, LINE_WORDS);
}

#[test]
fn a_strong_signal_with_offset_drift_and_clock_error_locks() {
    let r = go(12.0, 40.0, 2500.0, 30.0, 80.0);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("lines") >= 15.0, "{}", r.num("lines"));
    assert!(
        (r.num("offset_hz") - 2860.0).abs() < 400.0,
        "{}",
        r.num("offset_hz")
    );
    // The picture of channel A is the test picture: its top 8 bits within 6 counts in 9 pixels of 10.
    let strips: Vec<Strip> = r
        .of::<Strip>()
        .into_iter()
        .filter(|s| s.instrument == CHANNEL_A)
        .collect();
    assert!(strips.len() >= 15);
    let s = &strips[strips.len() - 2];
    let fits = |line: u32| {
        (0..PICTURE_WORDS)
            .filter(|&x| {
                let want = i32::from(test_value(0, x as u32, line) >> 2);
                (i32::from(s.samples[x]) - want).abs() <= 6
            })
            .count()
    };
    let best = (0..400).map(fits).max().unwrap_or(0);
    assert!(best >= PICTURE_WORDS * 9 / 10, "{best} of {PICTURE_WORDS}");
    assert!(r.of::<Strip>().iter().any(|s| s.instrument == FULL));
}

#[test]
fn a_weak_signal_gives_lines() {
    let r = go(10.0, 14.0, -3000.0, -40.0, -60.0);
    assert!(r.num("lines") >= 10.0, "{}", r.num("lines"));
}

#[test]
fn noise_gives_no_lock() {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    let x: Vec<Complex32> = (0..(30.0 * 240e3) as usize)
        .map(|_| Complex32::new(next() + next(), next() + next()) * 0.2)
        .collect();
    let h = Harness::new(&DESCRIPTOR).rate(240e3);
    let r = run::<Strip>(&h, &x, 240e3, 137.1e6, 137.1e6, 65_536, Some("apt.out"));
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    assert_eq!(r.num("lines"), 0.0);
    assert!(r.of::<Strip>().is_empty());
}

#[test]
fn the_lock_ends_in_noise_and_comes_back_with_the_signal() {
    let rate = 240e3;
    let h = Harness::new(&DESCRIPTOR)
        .rate(rate)
        .seconds(5.0)
        .set(&GEN_LEVEL_DBFS, -13.0);
    let x = h.generate().expect("a signal");
    let r = run::<Strip>(&h, &x, rate, 137.1e6, 137.1e6, 65_536, None);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    let lines = r.num("lines");
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 40) as f32 / (1u64 << 24) as f32 - 0.5
    };
    let noise: Vec<Complex32> = (0..(3.5 * rate) as usize)
        .map(|_| Complex32::new(next() + next(), next() + next()) * 0.2)
        .collect();
    let r = super::common::feed::<Strip>(r.rx, &noise, 65_536, None);
    assert_ne!(r.results.status().lock_state, LockState::Locked);
    let r = super::common::feed::<Strip>(r.rx, &x[..(3.0 * rate) as usize], 65_536, None);
    assert_eq!(r.results.status().lock_state, LockState::Locked);
    assert!(r.num("lines") > lines);
}

/// The PSNR (dB) of the strips of `instrument` (channel 0 for A, 1 for B) against the test picture after one least-squares
/// gain and offset; each strip is matched to its line of the picture. The first four strips (the lock settling) are left out.
fn psnr(strips: &[Strip], channel: u16) -> f64 {
    let want = |x: usize, line: u32| f64::from(test_value(channel, x as u32, line) >> 2);
    let mut pairs = Vec::new();
    for s in strips.iter().skip(4) {
        let err = |line: u32| -> f64 {
            (0..PICTURE_WORDS)
                .map(|x| (f64::from(s.samples[x]) - want(x, line)).powi(2))
                .sum()
        };
        let line = (0..400)
            .min_by(|&a, &b| err(a).total_cmp(&err(b)))
            .unwrap_or(0);
        pairs.extend((0..PICTURE_WORDS).map(|x| (want(x, line), f64::from(s.samples[x]))));
    }
    let n = pairs.len() as f64;
    let (mx, my) = (
        pairs.iter().map(|p| p.0).sum::<f64>() / n,
        pairs.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let sxy: f64 = pairs.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let sxx: f64 = pairs.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let g = sxy / sxx.max(1e-9);
    let mse = pairs
        .iter()
        .map(|p| (p.1 - (my + g * (p.0 - mx))).powi(2))
        .sum::<f64>()
        / n;
    10.0 * (255.0f64.powi(2) / mse.max(1e-9)).log10()
}

/// The line filter (click removal and the line Wiener filter on images A and B, with the noise of space A): with a 2.5 kHz
/// offset, 30 Hz/s of drift and an 80 ppm clock, the PSNR of images A and B at 30, 15, 10, 5, 2 and 0 dB in 40 kHz is at
/// least 34.1, 32.8, 17.3, 15.2, 12.8 and 11.0 dB.
#[test]
fn the_line_filter_raises_the_psnr() {
    let mut short = Vec::new();
    for (cn, need) in [
        (30.0, 34.1),
        (15.0, 32.8),
        (10.0, 17.3),
        (5.0, 15.2),
        (2.0, 12.8),
        (0.0, 11.0),
    ] {
        let r = go(14.0, cn, 2500.0, 30.0, 80.0);
        let strips = r.of::<Strip>();
        for (name, ch, instrument) in [("A", 0, CHANNEL_A), ("B", 1, CHANNEL_B)] {
            let s: Vec<Strip> = strips
                .iter()
                .filter(|s| s.instrument == instrument)
                .cloned()
                .collect();
            let p = if s.len() > 8 { psnr(&s, ch) } else { 0.0 };
            println!("{cn} dB, image {name}: {} lines, PSNR {p:.1} dB", s.len());
            if p < need {
                short.push(format!("{cn} dB, image {name}: {p:.1} < {need}"));
            }
        }
    }
    assert!(short.is_empty(), "{short:#?}");
}
