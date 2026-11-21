//! Link 11 CLEW: both frame rates and both step offsets through Doppler shifts, noise, a clock error and an echo; a fast and a
//! slow message in a row; noise alone.

use crate::common::*;
use sdrkit_algo_sim::fading::Profile;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::link11::{
    test_message, test_steps, Link11Gen, Message, LINK11, TEST_FRAMES,
};

fn signal(msgs: Vec<Message>) -> Vec<Complex32> {
    let secs: f64 = msgs
        .iter()
        .map(|m| (m.pre + 1 + m.steps.len()) as f64 * m.frame_s + m.gap_s)
        .sum();
    let mut x = vec![Complex32::default(); 7200];
    let mut g = Link11Gen::new(24e3, msgs);
    x.extend(generate(&mut g, secs + 0.2));
    x
}

fn digits(steps: &[[u8; 15]]) -> String {
    steps
        .iter()
        .map(|f| f.iter().map(|d| char::from(b'0' + d)).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

fn messages(out: &sdrkit_core::Output) -> Vec<(i64, String)> {
    out.events
        .iter()
        .filter(|e| e.kind() == "link11.message")
        .map(|e| {
            (
                e.at(&sdrkit_stack_hfdata::typed::Link11MessageEvent::DATA_FRAMES)
                    .unwrap_or(0),
                e.at(&sdrkit_stack_hfdata::typed::Link11MessageEvent::STEPS)
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect()
}

#[test]
fn both_rates_through_their_channels() {
    for rate in [0usize, 1] {
        for off in [0.0, 45.0] {
            let snr = if rate == 0 { 15.0 } else { 10.0 };
            let x = signal(vec![test_message(rate, off, 5, 7)]);
            let want = digits(&test_steps(TEST_FRAMES, 7));
            let cases: Vec<(&str, Channel, usize)> = vec![
                ("clean", Channel::default(), 4801),
                (
                    "+40 Hz",
                    Channel {
                        offset_hz: 40.0,
                        ..Channel::default()
                    },
                    4801,
                ),
                (
                    "-55 Hz",
                    Channel {
                        offset_hz: -55.0,
                        ..Channel::default()
                    },
                    4801,
                ),
                (
                    "noise",
                    Channel {
                        snr_db: Some(snr),
                        seed: 3,
                        ..Channel::default()
                    },
                    4801,
                ),
                (
                    "60 ppm",
                    Channel {
                        ppm: 60.0,
                        ..Channel::default()
                    },
                    4801,
                ),
                (
                    "echo",
                    Channel {
                        fading: Some(Profile::Echo {
                            delay_s: 0.001,
                            level_db: -10.0,
                            phase_deg: 40.0,
                        }),
                        ..Channel::default()
                    },
                    4801,
                ),
                ("pieces", Channel::default(), 77),
            ];
            for (name, ch, piece) in cases {
                let (rx, out) = run_24k(&LINK11, &through(&x, 24e3, &ch), piece);
                let m = messages(&out);
                assert_eq!(
                    m.len(),
                    1,
                    "{rate} {off} {name}: {:?}",
                    event_texts(&out, "link11.event")
                );
                assert_eq!(m[0].0, TEST_FRAMES as i64, "{rate} {off} {name}");
                assert_eq!(m[0].1, want, "{rate} {off} {name}");
                let d = rows(rx.results(), &sdrkit_stack_hfdata::typed::LINK11_MESSAGES);
                assert!(
                    (num(&d[0], "doppler_hz") - ch.offset_hz).abs() < 2.0,
                    "{rate} {off} {name}: {}",
                    num(&d[0], "doppler_hz")
                );
                let od = num(&d[0], "offset_deg");
                assert!(
                    (od - off).abs() <= 3.0 || (od - off).abs() >= 87.0,
                    "{rate} {off} {name}: {od}"
                );
            }
        }
    }
}

#[test]
fn a_fast_and_a_slow_message() {
    let x = signal(vec![
        test_message(0, 45.0, 5, 1),
        test_message(1, 45.0, 5, 2),
    ]);
    let (rx, out) = run_24k(&LINK11, &x, 4801);
    let m = messages(&out);
    assert_eq!(m.len(), 2);
    assert_eq!(m[0].1, digits(&test_steps(TEST_FRAMES, 1)));
    assert_eq!(m[1].1, digits(&test_steps(TEST_FRAMES, 2)));
    let d = rows(rx.results(), &sdrkit_stack_hfdata::typed::LINK11_MESSAGES);
    assert_eq!(text(&d[0], "rate"), "fast");
    assert_eq!(text(&d[1], "rate"), "slow");
}

#[test]
fn noise_gives_no_message() {
    let x = vec![Complex32::default(); 24_000 * 4];
    let ch = Channel {
        snr_db: Some(0.0),
        seed: 4,
        ..Channel::default()
    };
    let (_, out) = run_24k(&LINK11, &through(&x, 24e3, &ch), 4801);
    assert!(messages(&out).is_empty());
}
