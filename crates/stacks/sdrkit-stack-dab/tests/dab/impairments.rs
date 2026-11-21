//! Impairments added independently of the generator: noise, offsets, clock errors, echoes, fading, dropped samples, the
//! protection profiles, conditional access, the search in noise and the quiet time.
#![allow(clippy::needless_range_loop)]

use super::common::*;
use sdrkit_core::{Complex32, LockState, Owner, StreamKind};
use sdrkit_stack_dab::generator::content::Programme;
use sdrkit_stack_dab::generator::services::PadContent;
use sdrkit_stack_dab::generator::{GenParams, ServiceAudio, ServiceSpec};
use sdrkit_stack_dab::results::*;
use sdrkit_stack_dab::tables::Protection;
use sdrkit_stack_dab::typed::*;

const RATE: f64 = 2.048e6;

fn clean(seconds: f64) -> Vec<Complex32> {
    default_signal(GenParams::default(), Air::default(), seconds)
}

fn noise(x: &mut [Complex32], cn_db: f64, seed: u64) {
    awgn(x, 0.04, cn_db, RATE, RATE, seed);
}

#[test]
fn tuning_errors() {
    for off in [-12e3, 12e3, 40.5e3, -75e3, 75e3] {
        let mut x = clean(5.0);
        offset(&mut x, RATE, off);
        noise(&mut x, 40.0, 3);
        let r = run(&x, RATE, 1, 65_536);
        assert_good(&r, 0.99, &format!("{off} Hz"));
        let c = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
        assert!((c - off).abs() < 50.0, "{off}: {c}");
    }
}

#[test]
fn sample_clock_errors() {
    for ppm in [-100.0, -80.0, 80.0, 100.0] {
        let x = clean(6.0);
        let mut y = clock(&x, ppm);
        offset(&mut y, RATE, 3000.0);
        noise(&mut y, 40.0, 4);
        let r = run(&y, RATE, 1, 65_536);
        assert_good(&r, 0.99, &format!("{ppm} ppm"));
    }
}

#[test]
fn swapped_iq() {
    let mut x = clean(5.0);
    offset(&mut x, RATE, 2300.0);
    for v in x.iter_mut() {
        *v = Complex32::new(v.im, v.re);
    }
    noise(&mut x, 40.0, 5);
    let r = run(&x, RATE, 1, 65_536);
    assert_good(&r, 0.99, "swapped I/Q");
    assert_eq!(r.at(&SyncFields::MIRRORED), Some(true));
}

#[test]
fn uep_profiles_for_dab_plus() {
    for idx in [5u8, 7, 9] {
        let mut p = GenParams::default();
        p.services[0].protection = Protection::Uep(idx);
        let mut x = generate_air(
            p,
            Air {
                offset_hz: -7300.0,
                ..Air::cn(40.0)
            },
            5.0,
        );
        quantize8(&mut x);
        let r = run(&x, RATE, 1, 65_536);
        assert_good(&r, 0.99, &format!("UEP {idx}"));
        assert_eq!(
            r.at(&PlayingFields::SELECTED_PROTECTION)
                .map(str::to_string),
            Some(format!("UEP {idx}"))
        );
    }
}

#[test]
fn reconfiguration() {
    let a = GenParams::default();
    let mut b = a.clone();
    // The selected sub-channel moves: the services are laid out in another order.
    b.services.swap(0, 3);
    let mut x = generate_air(a, Air::cn(40.0), 6.0);
    let y = generate_air(b, Air::cn(40.0), 10.0);
    // Continue at a frame boundary.
    let cut = (x.len() / 196_608) * 196_608;
    x.truncate(cut);
    x.extend_from_slice(&y);
    let r = run(&x, RATE, 1, 65_536);
    assert!(fib_share(&r) >= 0.97, "{}", fib_share(&r));
    let ok = r.count(&AudioFields::AUDIO_UNITS_OK);
    assert!(ok > 600, "{ok}");
    let sub1 = r
        .rows(&SUBCHANNELS)
        .into_iter()
        .find(|(k, _)| *k == "1")
        .unwrap()
        .1;
    assert_eq!(sub1.at(&SubchannelRow::START_CU), Some(168));
}

#[test]
fn off_centre() {
    // +190 kHz at 2.048 Msps with a DC spike 3 dB below the signal.
    let mut x = clean(5.0);
    offset(&mut x, RATE, 190e3);
    let dc = Complex32::new((0.04f32 / 2.0).sqrt(), 0.0);
    for v in x.iter_mut() {
        *v += dc;
    }
    noise(&mut x, 40.0, 6);
    let r = run(&x, RATE, 1, 65_536);
    assert_good(&r, 0.95, "+190 kHz");
    let c = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
    assert!((c - 190e3).abs() < 50.0, "{c}");
    // −1.7 MHz at 8 Msps.
    let mut x = default_signal(
        GenParams {
            rate_hz: 8e6,
            ..Default::default()
        },
        Air::default(),
        5.0,
    );
    offset(&mut x, 8e6, -1.7e6);
    awgn(&mut x, 0.04 * 2.048 / 8.0, 40.0, 2.048e6, 8e6, 7);
    let r = run(&x, 8e6, 1, 65_536);
    assert_good(&r, 0.95, "-1.7 MHz at 8 Msps");
}

#[test]
fn dc_spike_at_signal_level() {
    let mut x = clean(5.0);
    let dc = Complex32::new(0.2, 0.0);
    for v in x.iter_mut() {
        *v += dc;
    }
    noise(&mut x, 40.0, 8);
    let r = run(&x, RATE, 1, 65_536);
    assert_good(&r, 0.99, "DC");
}

#[test]
fn samples_not_finite() {
    let mut x = clean(5.0);
    noise(&mut x, 40.0, 9);
    for (i, v) in x[3_000_000..3_004_000].iter_mut().enumerate() {
        *v = if i % 2 == 0 {
            Complex32::new(f32::NAN, 0.0)
        } else {
            Complex32::new(f32::INFINITY, -f32::INFINITY)
        };
    }
    let r = run(&x, RATE, 1, 65_536);
    assert_good(&r, 0.95, "NaN");
    assert!(r.count(&sdrkit_graph::GuardFields::BAD_SAMPLES) >= 4000);
}

#[test]
fn samples_lost() {
    let mut x = clean(5.6);
    noise(&mut x, 40.0, 10);
    let mut y: Vec<Complex32> = x[100_000..].to_vec();
    y.drain(9_000_000 - 100_000..9_000_000 - 100_000 + 777);
    y.drain(2_500_000..2_500_000 + 12_345);
    let r = run(&y, RATE, 1, 65_536);
    assert_good(&r, 0.95, "lost samples");
    assert!(r.count(&SyncFields::SYNC_LOSSES) >= 1);
}

#[test]
fn combined() {
    let x = clean(6.0);
    let mut y = clock(&x, 80.0);
    echo(&mut y, 300, -6.0);
    offset(&mut y, RATE, 75e3);
    noise(&mut y, 40.0, 11);
    quantise(&mut y, 8, 2.5 * 0.2);
    let r = run(&y, RATE, 1, 65_536);
    assert_good(&r, 0.97, "combined");
    let c = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
    assert!((c - 75e3).abs() < 50.0, "{c}");
}

fn mp2_ensemble(bitrate: u16, protection: Protection, rate: u32, stereo: bool) -> GenParams {
    GenParams {
        services: vec![ServiceSpec {
            sid: 0xC201,
            label: "Layer II".into(),
            subch: 5,
            bitrate,
            protection,
            audio: ServiceAudio::Mp2 { rate, stereo },
            programme: Programme::Tones(1500.0, 750.0),
            pad: PadContent::default(),
            ca: false,
            extra_components: Vec::new(),
        }],
        ..Default::default()
    }
}

fn check_mp2(p: GenParams, air: Air, what: &str) {
    let sent = sdrkit_stack_dab::generator::DabGenerator::new(p.clone());
    let lfs: Vec<Vec<u8>> = sent.logical_frames(5).unwrap().to_vec();
    let x = generate_air(p, air, 4.0);
    let r = run_sid(&x, RATE, "C201", 65_536);
    assert_eq!(r.lock_state(), LockState::Locked, "{what}");
    let frames = r.units(StreamKind::Mp2);
    assert!(frames.len() > 50, "{what}: {} frames", frames.len());
    let lf_total = r.count(&AudioFields::AUDIO_LOGICAL_FRAMES);
    assert!(lf_total > 100, "{what}: {lf_total} logical frames");
    // Every frame out is the transmitter's, to the bit.
    let cat: Vec<u8> = lfs.concat();
    for f in &frames {
        let b = f.packet.bytes();
        assert!(
            cat.windows(b.len()).step_by(lfs[0].len()).any(|w| w == b),
            "{what}: a frame that was not sent"
        );
    }
    let l = left(&r.audio);
    assert!(l.len() > 48_000, "{what}");
    let dc = l.iter().map(|v| *v as f64).sum::<f64>() / l.len() as f64;
    assert!(dc.abs() < 0.01, "{what}: DC {dc}");
    let step_tone = 0.5 * std::f64::consts::TAU * 1500.0 / 48_000.0;
    let max_step = l
        .windows(2)
        .map(|w| (w[1] - w[0]).abs() as f64)
        .fold(0.0, f64::max);
    assert!(max_step < 1.6 * step_tone, "{what}: step {max_step}");
    let dur = frames.len() as f64
        * if frames[0].format.audio.as_ref().unwrap().sample_rate == 24_000 {
            0.048
        } else {
            0.024
        };
    assert!(
        l.len() as f64 / 48_000.0 <= dur + 0.3,
        "{what}: too much audio"
    );
}

#[test]
fn every_uep_profile_with_layer_ii() {
    for idx in 0..64usize {
        let u = sdrkit_stack_dab::tables::UEP[idx];
        let stereo = !matches!(u.bitrate, 32 | 48 | 56 | 80);
        check_mp2(
            mp2_ensemble(u.bitrate, Protection::Uep(idx as u8), 48_000, stereo),
            Air::default(),
            &format!("UEP {idx}"),
        );
    }
    for (br, p, stereo) in [
        (
            128,
            Protection::Eep {
                option: 0,
                level: 3,
            },
            true,
        ),
        (
            192,
            Protection::Eep {
                option: 0,
                level: 1,
            },
            true,
        ),
        (
            128,
            Protection::Eep {
                option: 1,
                level: 2,
            },
            true,
        ),
        (
            96,
            Protection::Eep {
                option: 1,
                level: 4,
            },
            false,
        ),
    ] {
        check_mp2(
            mp2_ensemble(br, p, 48_000, stereo),
            Air::default(),
            &format!("{br} {p:?}"),
        );
    }
    check_mp2(
        mp2_ensemble(
            64,
            Protection::Eep {
                option: 0,
                level: 3,
            },
            24_000,
            false,
        ),
        Air::default(),
        "24 kHz EEP 3-A",
    );
    check_mp2(
        mp2_ensemble(64, Protection::Uep(15), 24_000, false),
        Air::default(),
        "24 kHz UEP 15",
    );
    let p = mp2_ensemble(128, Protection::Uep(36), 48_000, true);
    check_mp2(p, Air::cn(8.0), "UEP 36 at 8 dB");
}

#[test]
fn conditional_access() {
    let mut p = GenParams::default();
    p.services[0].ca = true;
    let x = generate_air(p, Air::cn(40.0), 4.0);
    let r = run(&x, RATE, 1, 65_536);
    assert_eq!(r.at(&PlayingFields::PLAYING), Some(Playing::Scrambled));
    assert!(r.units(StreamKind::Aac).is_empty());
    let c101 = r
        .rows(&SERVICES)
        .into_iter()
        .find(|(k, _)| *k == "C101")
        .unwrap()
        .1;
    assert_eq!(c101.at(&ServiceRow::SCRAMBLED), Some(true));
    let r = run(&x, RATE, 2, 65_536);
    assert!(r.units(StreamKind::Aac).len() > 50);
}

#[test]
fn search_load_and_lock_after_noise() {
    let mut x: Vec<Complex32> = vec![Complex32::default(); (4.0 * RATE) as usize];
    awgn(&mut x, 0.04, 0.0, RATE, RATE, 12);
    for (i, v) in x.iter_mut().enumerate() {
        if i % 5000 < 300 || i % 50_000 < 3000 {
            *v *= 0.05;
        }
    }
    let t = std::time::Instant::now();
    let r = run(&x, RATE, -1, 65_536);
    let el = t.elapsed().as_secs_f64();
    eprintln!("4 s of dipping noise searched in {el:.2} s");
    assert_ne!(r.lock_state(), LockState::Locked);
    // A DAB signal after it locks within 3 s.
    let mut y = x.clone();
    y.extend(clean(3.0));
    let r = run(&y, RATE, -1, 65_536);
    assert_eq!(r.lock_state(), LockState::Locked);
    let lock = r.at(&TimingFields::TIMING_LOCK).unwrap();
    assert!(lock < 4.0 + 3.0, "lock at {lock}");
}

#[test]
fn quiet_time() {
    let mut x: Vec<Complex32> = vec![Complex32::default(); RATE as usize];
    awgn(&mut x, 0.04, 0.0, RATE, RATE, 13);
    let r = run(&x, RATE, -1, 20_480);
    let q = r.at(&SyncFields::QUIET).unwrap_or(0.0) * 1000.0;
    assert!(q > 700.0, "noise: {q} ms quiet");
    for cn in [0.0, 3.0] {
        let x = default_signal(GenParams::default(), Air::cn(cn), 2.0);
        let mut max_q = 0f64;
        let mut rx = receiver(&[("service", -1.0)], RATE);
        let mut out = sdrkit_core::Output::default();
        for c in x.chunks(20_480) {
            out.clear();
            sdrkit_core::Receiver::feed(&mut rx, c, &mut out);
            max_q = max_q.max(rx.results().fields().at(&SyncFields::QUIET).unwrap_or(0.0) * 1000.0);
        }
        assert!(max_q <= 300.0, "{cn} dB: {max_q} ms quiet");
    }
}
