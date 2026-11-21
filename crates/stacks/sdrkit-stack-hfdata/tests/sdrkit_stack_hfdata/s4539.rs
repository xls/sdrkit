//! STANAG 4539: the test signal, every rate through noise at the levels of the target quality, the HF fading channels of
//! ITU-R F.520, a join at a reinserted block, two messages of their own rates in a row, noise alone.

use crate::common::*;
use sdrkit_algo_sim::fading::profiles::Watterson;
use sdrkit_algo_sim::fading::Profile;
use sdrkit_core::Complex32;
use sdrkit_stack_hfdata::s4539::signal::Rate;
use sdrkit_stack_hfdata::s4539::{self, S4539};
use sdrkit_stack_hfdata::serial::gen::SymbolGen;
use sdrkit_stack_hfdata::serial::TEST_TEXT;

fn messages(out: &sdrkit_core::Output) -> Vec<String> {
    event_texts(out, "s4539.message")
}

/// One transmission of `bytes` at 24 kHz with 0.3 s of silence before and after.
fn transmission(rate: Rate, frames: usize, bytes: &[u8]) -> Vec<Complex32> {
    let mut sym = vec![Complex32::default(); 720];
    sym.extend(s4539::test_symbols(rate, frames, 0, bytes, true));
    sym.extend(vec![Complex32::default(); 720]);
    let n = sym.len() * 10;
    let mut g = SymbolGen::serial(24e3, sym);
    generate(&mut g, n as f64 / 24e3)
}

fn read(x: &[Complex32], ch: &Channel, eq: bool) -> (Vec<String>, Vec<String>) {
    let mut rx = receiver(&S4539, &[("equaliser", json!(eq))]);
    let f = S4539.tuning.default_hz;
    rx.configure(&sdrkit_core::InputInfo::new(24e3, f, f).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), &through(x, 24e3, ch), 4801);
    out.events.append(&mut end(rx.as_mut()).events);
    (messages(&out), event_texts(&out, "s4539.event"))
}

#[test]
fn the_test_signal_is_read() {
    let x = test_signal(&S4539, 24e3, &[], 6.0);
    let (rx, out) = run_24k(&S4539, &x, 4801);
    let m = messages(&out);
    assert!(m.len() >= 2, "{m:?} {:?}", event_texts(&out, "s4539.event"));
    assert!(m.iter().all(|t| t == TEST_TEXT), "{m:?}");
    let ev = event_texts(&out, "s4539.event");
    assert!(
        ev.iter().any(|e| e.starts_with(
            "transmission start: 4800 bit/s, very short interleaver (3 frames), frequency error"
        )),
        "{ev:?}"
    );
    assert!(field(rx.as_ref(), "messages_total").unwrap() >= 2.0);
}

#[test]
fn every_rate_through_noise() {
    let cases = [
        (Rate::B3200, 9, 11.0),
        (Rate::B4800, 9, 15.0),
        (Rate::B6400, 9, 19.0),
        (Rate::B8000, 9, 22.0),
        (Rate::B9600, 9, 24.0),
        (Rate::B12800, 1, 35.0),
    ];
    for (rate, frames, snr) in cases {
        let x = transmission(rate, frames, TEST_TEXT.as_bytes());
        let ch = Channel {
            snr_db: Some(snr),
            offset_hz: 37.0,
            seed: 21,
            ..Channel::default()
        };
        let (m, ev) = read(&x, &ch, true);
        assert!(
            !m.is_empty() && m[0] == TEST_TEXT,
            "{rate:?} at {snr} dB: {m:?} {ev:?}"
        );
    }
}

#[test]
fn hf_fading_channels() {
    let text: Vec<u8> = TEST_TEXT.bytes().cycle().take(600).collect();
    let want = String::from_utf8(text.clone()).unwrap();
    let cases = [
        (Rate::B3200, 9, Watterson::Poor, 15.0, 31),
        (Rate::B6400, 9, Watterson::Moderate, 26.0, 32),
        (Rate::B9600, 9, Watterson::Poor, 36.0, 33),
    ];
    for (rate, frames, w, snr, seed) in cases {
        let x = transmission(rate, frames, &text);
        let ch = Channel {
            snr_db: Some(snr),
            fading: Some(Profile::Watterson(w)),
            seed,
            ..Channel::default()
        };
        let (m, ev) = read(&x, &ch, true);
        let bad: Vec<usize> = m
            .first()
            .map(|g| {
                g.bytes()
                    .zip(want.bytes())
                    .enumerate()
                    .filter(|(_, (a, b))| a != b)
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            !m.is_empty() && m[0] == want,
            "{rate:?} {w:?}: {} messages, {} bytes, wrong at {bad:?} {ev:?}",
            m.len(),
            m.first().map_or(0, |g| g.len())
        );
    }
    // Without the equaliser, ITU-R Poor is not read.
    let x = transmission(Rate::B3200, 9, &text);
    let ch = Channel {
        snr_db: Some(15.0),
        fading: Some(Profile::Watterson(Watterson::Poor)),
        seed: 31,
        ..Channel::default()
    };
    let (m, _) = read(&x, &ch, false);
    assert!(m.first() != Some(&want));
}

#[test]
fn a_join_at_a_reinserted_block() {
    // A transmission longer than a superframe, its first 3 s missed.
    let text: Vec<u8> = TEST_TEXT.bytes().cycle().take(9000).collect();
    let x = transmission(Rate::B4800, 9, &text);
    let (m, ev) = read(&x[72_000..], &Channel::default(), true);
    assert!(
        ev.iter()
            .any(|e| e.starts_with("joined at a reinserted preamble")),
        "{ev:?}"
    );
    assert_eq!(m.len(), 1, "{ev:?}");
    let got = m[0].as_bytes();
    // The joined message is the end of the text.
    assert!(got.len() > 3000);
    assert_eq!(got, &text[text.len() - got.len()..]);
}

#[test]
fn two_messages_of_their_own_rates() {
    let mut x = transmission(Rate::B3200, 3, b"FIRST MESSAGE AT 3200");
    x.extend(transmission(Rate::B9600, 1, b"SECOND MESSAGE AT 9600"));
    let ch = Channel {
        snr_db: Some(30.0),
        seed: 41,
        ..Channel::default()
    };
    let (m, ev) = read(&x, &ch, true);
    assert_eq!(
        m,
        ["FIRST MESSAGE AT 3200", "SECOND MESSAGE AT 9600"],
        "{ev:?}"
    );
}

#[test]
fn noise_never_locks() {
    let x = vec![Complex32::default(); 24_000 * 8];
    for seed in [1, 2] {
        let ch = Channel {
            snr_db: Some(0.0),
            seed,
            ..Channel::default()
        };
        let (m, ev) = read(&x, &ch, true);
        assert!(m.is_empty() && ev.is_empty(), "{ev:?}");
    }
}

#[test]
fn offsets_agc_blocks_an_echo_and_a_clock_error() {
    let text: Vec<u8> = TEST_TEXT.bytes().cycle().take(1500).collect();
    let want = String::from_utf8(text.clone()).unwrap();
    let mut agc = vec![Complex32::default(); 720];
    agc.extend(s4539::test_symbols(Rate::B4800, 1, 3, &text, true));
    agc.extend(vec![Complex32::default(); 720]);
    let n = agc.len() * 10;
    let x_agc = generate(&mut SymbolGen::serial(24e3, agc), n as f64 / 24e3);
    let cases = [
        (transmission(Rate::B9600, 9, &text), -70.0, 30.0, None, 0.0),
        (transmission(Rate::B4800, 3, &text), 70.0, 20.0, None, 0.0),
        (x_agc, 10.0, 25.0, None, 0.0),
        (
            transmission(Rate::B3200, 9, &text),
            0.0,
            20.0,
            Some(Profile::Echo {
                delay_s: 0.001,
                level_db: -12.0,
                phase_deg: 30.0,
            }),
            0.0,
        ),
        (transmission(Rate::B6400, 9, &text), 0.0, 30.0, None, 40.0),
        (transmission(Rate::B6400, 9, &text), 0.0, 30.0, None, -40.0),
    ];
    for (i, (x, off, snr, fading, ppm)) in cases.into_iter().enumerate() {
        let ch = Channel {
            snr_db: Some(snr),
            offset_hz: off,
            fading,
            ppm,
            seed: 50 + i as u64,
            ..Channel::default()
        };
        let (m, ev) = read(&x, &ch, true);
        assert!(
            !m.is_empty() && m[0] == want,
            "case {i}: {} {ev:?}",
            m.len()
        );
    }
}

#[test]
fn a_very_long_interleaver_through_itu_poor() {
    let text: Vec<u8> = TEST_TEXT.bytes().cycle().take(3000).collect();
    let want = String::from_utf8(text.clone()).unwrap();
    let x = transmission(Rate::B3200, 72, &text);
    let ch = Channel {
        snr_db: Some(15.0),
        fading: Some(Profile::Watterson(Watterson::Poor)),
        seed: 77,
        ..Channel::default()
    };
    let (m, ev) = read(&x, &ch, true);
    assert!(!m.is_empty() && m[0] == want, "{} {ev:?}", m.len());
}

/// The frequency error in the start event, Hz.
fn start_offset(ev: &[String]) -> Option<f64> {
    let e = ev.iter().find(|e| e.starts_with("transmission start"))?;
    let v = e.split("frequency error ").nth(1)?.split(" Hz").next()?;
    v.parse().ok()
}

/// The acquisition range is +-96 Hz, through noise and through ITU-R Poor.
#[test]
fn acquisition_at_the_edges_of_the_range() {
    let text: Vec<u8> = TEST_TEXT.bytes().cycle().take(600).collect();
    let want = String::from_utf8(text.clone()).unwrap();
    let x = transmission(Rate::B3200, 9, &text);
    for off in [-96.0, -90.0, 90.0, 96.0] {
        for (fading, snr, seed) in [
            (None, 20.0, 61),
            (Some(Profile::Watterson(Watterson::Poor)), 15.0, 31),
            (Some(Profile::Watterson(Watterson::Poor)), 15.0, 35),
        ] {
            let ch = Channel {
                snr_db: Some(snr),
                offset_hz: off,
                fading: fading.clone(),
                seed,
                ..Channel::default()
            };
            let (m, ev) = read(&x, &ch, true);
            let f = start_offset(&ev);
            assert!(
                f.is_some_and(|f| (f - off).abs() < 3.0),
                "{off} Hz {fading:?}: {ev:?}"
            );
            assert!(
                !m.is_empty() && m[0] == want,
                "{off} Hz {fading:?}: {} {ev:?}",
                m.len()
            );
        }
    }
}
