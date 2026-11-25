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

#[test]
fn gen_lines_makes_a_pass_of_that_many_lines_and_the_picture_ends_finished() {
    use sdrkit_stack_satimg::modes::apt::GEN_LINES;
    // 30 lines are 15 s. The harness asks for more, and the signal ends by itself.
    let h = Harness::new(&DESCRIPTOR)
        .rate(RATE)
        .seconds(40.0)
        .set(&GEN_LEVEL_DBFS, -13.0)
        .set(&GEN_LINES, 30);
    let x = h.generate().expect("a signal");
    assert_eq!(x.len(), (15.0 * RATE) as usize);
    let mut r = run::<Strip>(&h, &x, RATE, 137.1e6, 137.1e6, 65_536, Some("apt.out"));
    let lines = r.num("lines");
    assert!((24.0..=30.0).contains(&lines), "{lines}");
    // The end of the input ends the picture: its last state is finished.
    let mut out = sdrkit_core::Output::default();
    r.rx.finish(&mut out);
    let last = out
        .media
        .iter()
        .rev()
        .find_map(|e| e.picture.as_ref())
        .expect("a picture at the end of the pass");
    assert_eq!(last.state, sdrkit_core::PictureState::Finished);
    // Without the setting the signal has no end.
    let h = Harness::new(&DESCRIPTOR).rate(RATE).seconds(16.0);
    assert_eq!(
        h.generate().expect("a signal").len(),
        (16.0 * RATE) as usize
    );
}

/// The APT test signal of `lines` lines at 120 kHz.
fn apt_pass(lines: i64) -> Vec<Complex32> {
    use sdrkit_stack_satimg::modes::apt::GEN_LINES;
    Harness::new(&DESCRIPTOR)
        .rate(120e3)
        .seconds(60.0)
        .set(&GEN_LEVEL_DBFS, -13.0)
        .set(&GEN_LINES, lines)
        .generate()
        .expect("a signal")
}

/// A receiver at 120 kHz whose pass ends after 5 s of signal without a line.
fn apt_receiver() -> Box<dyn sdrkit_core::Receiver> {
    use sdrkit_stack_satimg::components::pass::PASS_GAP;
    let mut rx = Harness::new(&DESCRIPTOR)
        .set(&PASS_GAP, 5.0)
        .receiver()
        .expect("a receiver");
    rx.configure(&sdrkit_core::InputInfo::new(120e3, 137.1e6, 137.1e6).recording())
        .expect("the rate");
    rx
}

#[test]
fn a_pass_ends_after_the_gap_and_the_next_pass_is_a_new_picture() {
    use super::common::{finished, noise, pictures_of};
    // Two passes of 20 lines (10 s each) with 8 s of noise between them: the gap of 5 s ends the first pass.
    let pass = apt_pass(20);
    assert_eq!(pass.len(), 1_200_000);
    let mut x = pass.clone();
    x.extend(noise(960_000, 0.02, 7));
    let first_end = x.len();
    x.extend(&pass);
    let mut rx = apt_receiver();
    let before = pictures_of(&mut *rx, &x[..first_end], 12_000, false);
    // The first pass is finished in the noise, before the second pass and before the end of the input.
    let done = finished(&before);
    assert_eq!(done.len(), 1, "{before:#?}");
    let first = before
        .iter()
        .find(|s| s.id == done[0])
        .expect("the first pass");
    assert!(first.name.contains("pass 1"), "{}", first.name);
    let after = pictures_of(&mut *rx, &x[first_end..], 12_000, true);
    let second: Vec<_> = after.iter().filter(|s| s.id != done[0]).collect();
    assert!(!second.is_empty(), "{after:#?}");
    assert!(
        second.iter().all(|s| s.name.contains("pass 2")),
        "{second:#?}"
    );
    assert!(
        second.iter().all(|s| s.name.starts_with("NOAA APT-")),
        "{second:#?}"
    );
    // The second pass ends at the end of the input.
    assert_eq!(finished(&after), vec![second[0].id]);
}

#[test]
fn a_short_dropout_stays_in_the_picture() {
    use super::common::{finished, noise, pictures_of};
    // 2 s of noise inside a pass is shorter than the gap of 5 s: one picture, finished only at the end of the input.
    let pass = apt_pass(20);
    let mut x = pass.clone();
    x.extend(noise(240_000, 0.02, 9));
    x.extend(&pass);
    let mut rx = apt_receiver();
    let sent = pictures_of(&mut *rx, &x, 12_000, true);
    let ids: std::collections::BTreeSet<u64> = sent.iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), 1, "{sent:#?}");
    assert_eq!(finished(&sent).len(), 1);
    assert_eq!(
        sent.last().map(|s| s.state),
        Some(sdrkit_core::PictureState::Finished)
    );
}

#[test]
fn a_pass_of_the_test_signal_ends_finished_in_the_silence_after_it() {
    use super::common::{finished, pictures_of};
    // The test-signal driver plays silence after the signal.
    let mut x = apt_pass(20);
    x.extend(vec![Complex32::default(); 720_000]);
    let mut rx = apt_receiver();
    let sent = pictures_of(&mut *rx, &x, 12_000, false);
    assert_eq!(finished(&sent).len(), 1, "{sent:#?}");
    assert_eq!(
        sent.last().map(|s| s.state),
        Some(sdrkit_core::PictureState::Finished)
    );
}

#[test]
fn with_live_samples_a_picture_is_named_by_the_utc_start_of_its_pass() {
    use super::common::pictures_of;
    use sdrkit_stack_satimg::components::pass::PASS_GAP;
    // Live samples: the host's clock gives the time of the signal.
    let mut rx = Harness::new(&DESCRIPTOR)
        .set(&PASS_GAP, 5.0)
        .clock(sdrkit_core::Clock::fixed(1_780_272_000.0 + 3_725.0))
        .receiver()
        .expect("a receiver");
    rx.configure(&sdrkit_core::InputInfo::new(120e3, 137.1e6, 137.1e6))
        .expect("the rate");
    let sent = pictures_of(&mut *rx, &apt_pass(20), 12_000, true);
    assert!(!sent.is_empty());
    assert!(
        sent.iter()
            .all(|s| s.name == "NOAA APT-ch1, pass 1, 2026-06-01 01:02 UTC"),
        "{sent:#?}"
    );
}

#[test]
fn a_new_pass_starts_at_row_0() {
    use super::common::{feed, noise, pictures_of};
    let pass = apt_pass(20);
    let mut first = pass.clone();
    first.extend(noise(960_000, 0.02, 7));
    let mut rx = apt_receiver();
    pictures_of(&mut *rx, &first, 12_000, false);
    let r = feed::<Strip>(rx, &pass, 12_000, Some("apt.out"));
    let strips = r.of::<Strip>();
    assert_eq!(strips.first().map(|s| s.first_row), Some(0), "{:?}", strips.len());
    assert!(strips.iter().all(|s| s.first_row < 25));
}
