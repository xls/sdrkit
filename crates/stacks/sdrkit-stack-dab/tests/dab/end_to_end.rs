//! The receiver end to end on the generator's signal.
#![allow(clippy::needless_range_loop)]

use super::common::*;
use sdrkit_core::{LockState, StreamKind};
use sdrkit_stack_dab::generator::{content::NOTES, GenParams};
use sdrkit_stack_dab::results::*;
use sdrkit_stack_dab::typed::*;

#[test]
fn every_service_at_30_db() {
    let mut x = default_signal(GenParams::default(), Air::cn(30.0), 8.0);
    quantize8(&mut x);
    for (sel, rate, (fl, fr)) in [
        (1, 48_000.0, (1000.0, 3000.0)),
        (2, 32_000.0, (2000.0, 500.0)),
        (3, 48_000.0, (0.0, 0.0)),
        (4, 48_000.0, (1500.0, 750.0)),
    ] {
        let r = run(&x, 2.048e6, sel, 65_536);
        assert_eq!(r.lock_state(), LockState::Locked);
        assert!(fib_share(&r) > 0.99);
        assert_eq!(r.at(&EnsembleFields::EID), Some(0xC0DE));
        assert_eq!(r.at(&EnsembleFields::ENSEMBLE_LABEL), Some("Test Ensemble"));
        assert_eq!(
            r.at(&PlayingFields::SELECTED_SUBCHANNEL),
            Some(i64::from(sel))
        );
        if sel < 4 {
            assert_eq!(
                r.at(&PlayingFields::SELECTED_KIND),
                Some(ServiceKind::DabPlus)
            );
            assert_eq!(r.at(&AudioFields::AUDIO_CODEC), Some(AudioCodec::AacLc));
            assert_eq!(r.at(&AudioFields::AUDIO_RATE), Some(rate));
            assert_eq!(r.at(&AudioFields::AUDIO_CHANNELS), Some(2));
            assert_eq!(r.count(&AudioFields::AUDIO_SUPERFRAMES_BAD), 0);
            assert_eq!(r.count(&AudioFields::AUDIO_UNITS_BAD), 0);
            assert_eq!(r.count(&AudioFields::AUDIO_RS_CORRECTED), 0);
            assert!(r.count(&AudioFields::AUDIO_SUPERFRAMES_OK) > 50);
            let units = r.units(StreamKind::Aac);
            assert!(units.len() > 200);
            // Every access unit out is one the transmitter made.
            let sent = sdrkit_stack_dab::generator::DabGenerator::from_loop(
                GenParams::default(),
                default_loop(),
            );
            let lfs = sent.logical_frames(sel as u8).unwrap();
            let all: Vec<u8> = lfs.concat();
            for u in units.iter().take(50) {
                let b = u.packet.bytes();
                assert!(all
                    .windows(b.len().min(32))
                    .any(|w| w == &b[..b.len().min(32)]));
            }
            if let Some((pcm, rr)) = ffmpeg_decode(&units) {
                let (l, rgt) = (left(&pcm), right(&pcm));
                let n = rr as usize;
                let seg = |v: &[f32]| v[n / 2..n / 2 + 2 * n].to_vec();
                if sel == 3 {
                    // Melody: the strongest tone between 200 and 900 Hz is a note or its octave in 80 % of 0.3 s windows.
                    let w = (0.3 * rr as f64) as usize;
                    let mut good = 0;
                    let mut total = 0;
                    for c in l[n..].chunks(w).filter(|c| c.len() == w) {
                        total += 1;
                        let mut best = (0.0, 0.0);
                        for f10 in 2000..9000 {
                            let f = f10 as f64 / 10.0;
                            let a = tone(c, rr as f64, f);
                            if a > best.0 {
                                best = (a, f);
                            }
                        }
                        if NOTES
                            .iter()
                            .any(|&n| (best.1 - n).abs() < 4.0 || (best.1 - 2.0 * n).abs() < 4.0)
                        {
                            good += 1;
                        }
                    }
                    assert!(good * 10 >= total * 8, "melody {good}/{total}");
                } else {
                    let (lt, rt) = (seg(&l), seg(&rgt));
                    assert!((20.0 * (tone(&lt, rr as f64, fl) / 0.5).log10()).abs() < 0.3);
                    assert!((20.0 * (tone(&rt, rr as f64, fr) / 0.5).log10()).abs() < 0.3);
                    assert!(tone(&rt, rr as f64, fl) < 0.01 * 0.5);
                }
            }
        } else {
            assert_eq!(r.at(&PlayingFields::SELECTED_KIND), Some(ServiceKind::Dab));
            assert_eq!(r.at(&AudioFields::AUDIO_CODEC), Some(AudioCodec::Mp2));
            assert!(r.count(&AudioFields::AUDIO_MP2_FRAMES) > 250);
            let (l, rgt) = (left(&r.audio), right(&r.audio));
            assert!(l.len() > 4 * 48_000);
            let lt = &l[24_000..24_000 + 96_000];
            let rt = &rgt[24_000..24_000 + 96_000];
            assert!((20.0 * (tone(lt, 48e3, fl) / 0.5).log10()).abs() < 1.0);
            assert!((20.0 * (tone(rt, 48e3, fr) / 0.5).log10()).abs() < 1.0);
            assert!(tone(rt, 48e3, fl) < 0.01 * 0.5);
            // Every logical frame out is the transmitter's: the MP2 frames match the sent ones.
            let sent = sdrkit_stack_dab::generator::DabGenerator::from_loop(
                GenParams::default(),
                default_loop(),
            );
            let lfs = sent.logical_frames(4).unwrap();
            for u in r.units(StreamKind::Mp2).iter().take(100) {
                assert!(lfs.iter().any(|f| f.as_slice() == u.packet.bytes()));
            }
        }
    }
}

#[test]
fn rates() {
    for rate in [2.0e6, 2.048e6, 2.1e6, 2.4e6, 4e6, 8e6, 10e6, 20e6] {
        let x = default_signal(
            GenParams {
                rate_hz: rate,
                ..Default::default()
            },
            Air::cn(30.0),
            4.0,
        );
        let r = run(&x, rate, 1, 65_536);
        assert_eq!(r.lock_state(), LockState::Locked, "{rate}");
        assert!(fib_share(&r) > 0.995, "{rate}: {}", fib_share(&r));
        assert!(r.count(&AudioFields::AUDIO_SUPERFRAMES_OK) > 20, "{rate}");
        assert_eq!(r.count(&AudioFields::AUDIO_SUPERFRAMES_BAD), 0, "{rate}");
        assert_eq!(r.count(&AudioFields::AUDIO_UNITS_BAD), 0, "{rate}");
        assert!(
            r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0).abs() < 50.0,
            "{rate}: {}",
            r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0)
        );
    }
}

#[test]
fn cn_sweep() {
    let mut table = Vec::new();
    for cn in [
        25.0, 20.0, 15.0, 12.0, 10.0, 8.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0, 0.0, -2.0,
    ] {
        let x = default_signal(GenParams::default(), Air::cn(cn), 10.0);
        let r = run(&x, 2.048e6, 1, 65_536);
        let share = fib_share(&r);
        let ok = r.count(&AudioFields::AUDIO_UNITS_OK) as f64;
        let bad = r.count(&AudioFields::AUDIO_UNITS_BAD) as f64;
        table.push((
            cn,
            r.lock_state() == LockState::Locked,
            share,
            ok,
            bad,
            r.at(&SignalFields::SNR_DB).unwrap_or(0.0),
        ));
        if cn >= 6.0 {
            assert_eq!(r.lock_state(), LockState::Locked, "{cn}");
            assert!(share > 0.95, "{cn}: {share}");
            assert!(ok > 400.0 && bad * 20.0 < ok, "{cn}: {ok} {bad}");
            assert!(r
                .rows(&SERVICES)
                .iter()
                .all(|(_, s)| s.at(&ServiceRow::LABEL).is_some_and(|l| !l.is_empty())));
        }
        if cn <= -2.0 {
            assert!(share < 0.9, "{cn}: the FIC should not pass");
        }
    }
    eprintln!("C/N dB  state  FIB share  AUs ok  bad  SNR");
    for t in &table {
        eprintln!(
            "{:6.1}  {}  {:8.3}  {:6}  {:4}  {:5.1}",
            t.0, t.1, t.2, t.3, t.4, t.5
        );
    }
    let fic = table
        .iter()
        .filter(|t| t.2 >= 0.9)
        .map(|t| t.0)
        .fold(f64::MAX, f64::min);
    let audio = table
        .iter()
        .filter(|t| t.3 > 0.0 && t.4 * 10.0 <= t.3 + t.4)
        .map(|t| t.0)
        .fold(f64::MAX, f64::min);
    eprintln!("lowest C/N with a working FIC: {fic} dB; with working audio: {audio} dB");
}

#[test]
fn offsets_and_clock_errors() {
    for (rate, offsets) in [
        (
            2.048e6,
            vec![
                -5000.0, -2500.0, -2490.0, -1234.5, 0.0, 700.0, 2510.0, 4500.0,
            ],
        ),
        (10e6, vec![-4500.0, -1500.0, 1500.0, 3300.0, 5000.0]),
    ] {
        for off in offsets {
            let x = default_signal(
                GenParams {
                    rate_hz: rate,
                    ..Default::default()
                },
                Air {
                    offset_hz: off,
                    ..Air::cn(25.0)
                },
                3.0,
            );
            let r = run(&x, rate, 1, 65_536);
            assert_eq!(r.lock_state(), LockState::Locked, "{rate} {off}");
            assert!(fib_share(&r) > 0.995, "{rate} {off}: {}", fib_share(&r));
            assert_eq!(r.count(&AudioFields::AUDIO_UNITS_BAD), 0, "{rate} {off}");
            let c = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
            assert!((c - off).abs() < 25.0, "{rate} {off}: read {c}");
        }
    }
    for ppm in [-20.0, -5.0, 5.0, 20.0] {
        let x = default_signal(
            GenParams::default(),
            Air {
                offset_hz: 800.0,
                clock_ppm: ppm,
                ..Air::cn(25.0)
            },
            8.0,
        );
        let r = run(&x, 2.048e6, 1, 65_536);
        assert!(fib_share(&r) > 0.995, "{ppm}: {}", fib_share(&r));
        assert_eq!(r.count(&AudioFields::AUDIO_UNITS_BAD), 0, "{ppm}");
        assert_eq!(r.count(&AudioFields::AUDIO_SUPERFRAMES_BAD), 0, "{ppm}");
        let c = r.at(&SignalFields::OFFSET_HZ).unwrap_or(0.0);
        assert!((c - 800.0).abs() < 25.0, "{ppm}: {c}");
        let m = r.at(&SignalFields::CLOCK_PPM).unwrap_or(0.0);
        assert!((m - ppm).abs() < 3.0, "{ppm}: measured {m}");
    }
}

#[test]
fn any_piece_size_returns_the_same_result() {
    let x = default_signal(
        GenParams::default(),
        Air {
            offset_hz: 1234.0,
            clock_ppm: 8.0,
            ..Air::cn(25.0)
        },
        1.2,
    );
    let counts = |piece: usize| {
        let r = run(&x, 2.048e6, 1, piece);
        assert_eq!(r.lock_state(), LockState::Locked);
        assert_eq!(r.count(&SyncFields::FIB_BAD), 0);
        assert_eq!(r.at(&EnsembleFields::ENSEMBLE_VALID), Some(true));
        (
            r.count(&SyncFields::FRAMES),
            r.count(&SyncFields::FIB_OK),
            r.count(&AudioFields::AUDIO_UNITS_OK),
        )
    };
    let a = counts(65_536);
    for p in [4096, 7, 1] {
        assert_eq!(counts(p), a, "pieces of {p}");
    }
}

#[test]
fn start_anywhere_in_a_frame() {
    let x = default_signal(GenParams::default(), Air::default(), 1.7);
    for drop in [0usize, 1, 100, 700, 1500, 2600, 3500, 5000, 20_000, 77_777] {
        let y = &x[drop..drop + (1.5 * 2.048e6) as usize];
        let r = run(y, 2.048e6, -1, 65_536);
        assert_eq!(r.lock_state(), LockState::Locked, "drop {drop}");
        assert!(r.count(&SyncFields::FRAMES) >= 5, "drop {drop}");
        assert_eq!(r.count(&SyncFields::FIB_BAD), 0, "drop {drop}");
    }
}
