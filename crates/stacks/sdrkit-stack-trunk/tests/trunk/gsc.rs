//! GSC: the test pages through the receiver over rates, carriers, clocks, deviation and an inverted spectrum; the worked
//! example; the address functions; a batch; calls without a preamble; noise; signals that are not GSC; the table commands.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{Generator, InputInfo, LockState};
use sdrkit_dev_test::Awgn;
use sdrkit_stack_trunk::gsc::gen::{length_s, segments, GscGenerator, GscOptions, TxKind, TxPage};
use sdrkit_stack_trunk::typed::GSC_PAGES;
use sdrkit_stack_trunk::GSC;
use std::collections::BTreeMap;
use std::f64::consts::TAU;

const DIAL: f64 = 160e6;

#[test]
fn the_data_file_has_the_band() {
    let file = crate::ermes::plan_file("gsc.json");
    let band = &file.groups[0].bands[0];
    assert_eq!((band.start_hz, band.end_hz), (929e6, 932e6));
}

/// One pass of the signal of `o` at `rate`, the channel `channel_hz` from the centre.
fn signal(rate: f64, channel_hz: f64, o: &GscOptions) -> Vec<Complex32> {
    let o = GscOptions {
        gap_s: 0.5,
        ..o.clone()
    };
    let mut g = GscGenerator::new(rate, channel_hz, o.clone());
    let mut x = vec![Complex32::new(0.0, 0.0); (length_s(&segments(&o)) * rate) as usize];
    g.generate(&mut x);
    x
}

/// A receiver fed `x` at `rate`, the channel `channel_hz` from the centre.
fn receive(x: &[Complex32], rate: f64, channel_hz: f64) -> (Box<dyn sdrkit_core::Receiver>, Fed) {
    let mut rx = make(&GSC, &[]);
    configure(rx.as_mut(), rate, DIAL - channel_hz, channel_hz);
    let fed = feed(rx.as_mut(), x, 8192);
    let mut out = sdrkit_core::Output::default();
    rx.finish(&mut out);
    (rx, fed)
}

fn page_rows(rx: &dyn sdrkit_core::Receiver) -> Vec<BTreeMap<String, String>> {
    rows(results(rx), &GSC_PAGES)
}

/// The rows of the default test pages: tone, numeric, alpha, voice, voice end.
fn check_test_pages(rows: &[BTreeMap<String, String>], what: &str) {
    let kinds: Vec<&str> = rows.iter().map(|r| r["kind"].as_str()).collect();
    assert_eq!(
        kinds,
        [
            "Tone page",
            "Numeric page",
            "Alpha page",
            "Voice page",
            "Voice end"
        ],
        "{what}: {rows:?}"
    );
    assert_eq!(rows[0]["code"], "954853", "{what}");
    assert_eq!(rows[0]["function"], "1", "{what}");
    assert_eq!(rows[0]["preamble"], "3", "{what}");
    assert_eq!(rows[0]["call"], "single call", "{what}");
    assert_eq!(rows[1]["code"], "123456", "{what}");
    assert_eq!(rows[1]["text"], "5551234", "{what}");
    assert_eq!(rows[2]["text"], "CALL HQ", "{what}");
    assert_eq!(rows[3]["word"], "3061", "{what}");
    assert_eq!(rows[4]["word"], "2457", "{what}");
    let len: f64 = rows[4]["voice_s"].parse().unwrap();
    assert!((2.0..2.6).contains(&len), "{what}: voice length {len}");
}

#[test]
fn the_test_pages_over_rates_carriers_clocks_and_senses() {
    struct Case {
        what: &'static str,
        rate: f64,
        channel: f64,
        o: GscOptions,
    }
    let base = GscOptions::default();
    let cases = [
        Case {
            what: "250 ksps",
            rate: 250e3,
            channel: 0.0,
            o: base.clone(),
        },
        Case {
            what: "96 ksps",
            rate: 96e3,
            channel: 0.0,
            o: base.clone(),
        },
        Case {
            what: "1 Msps",
            rate: 1e6,
            channel: 0.0,
            o: base.clone(),
        },
        Case {
            what: "40 kHz off the centre",
            rate: 250e3,
            channel: 40e3,
            o: base.clone(),
        },
        Case {
            what: "+2.5 kHz drifting +150 Hz/s",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                offset_hz: 2500.0,
                drift_hz_s: 150.0,
                ..base.clone()
            },
        },
        Case {
            what: "-2.5 kHz drifting -150 Hz/s",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                offset_hz: -2500.0,
                drift_hz_s: -150.0,
                ..base.clone()
            },
        },
        Case {
            what: "clock +800 ppm",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                clock_ppm: 800.0,
                ..base.clone()
            },
        },
        Case {
            what: "clock -800 ppm",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                clock_ppm: -800.0,
                ..base.clone()
            },
        },
        Case {
            what: "clock drifting 120 ppm/s",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                clock_drift_ppm_s: 120.0,
                ..base.clone()
            },
        },
        Case {
            what: "deviation 3 kHz",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                deviation_hz: 3000.0,
                ..base.clone()
            },
        },
        Case {
            what: "inverted spectrum",
            rate: 250e3,
            channel: 0.0,
            o: GscOptions {
                inverted: true,
                ..base.clone()
            },
        },
    ];
    for c in cases {
        let x = signal(c.rate, c.channel, &c.o);
        let (rx, fed) = receive(&x, c.rate, c.channel);
        check_test_pages(&page_rows(rx.as_ref()), c.what);
        let pages = fed.events.iter().filter(|e| e.kind() == "gsc.page").count();
        assert_eq!(pages, 4, "{}", c.what);
        check_events(
            &kinds(&sdrkit_stack_trunk::gsc::declaration()),
            &fed.events,
            true,
        );
        let r = results(rx.as_ref());
        if c.what == "inverted spectrum" {
            assert_eq!(text(r, "inverted"), "yes");
        }
    }
}

#[test]
fn the_worked_example_and_the_address_functions() {
    let mut pages = vec![TxPage::new(954_853, TxKind::Tone, "")];
    for f in 2..=4 {
        pages.push(TxPage {
            function: f,
            ..TxPage::new(123_456, TxKind::Tone, "")
        });
    }
    let o = GscOptions {
        pages,
        ..GscOptions::default()
    };
    let (rx, _) = receive(&signal(250e3, 0.0, &o), 250e3, 0.0);
    let rows = page_rows(rx.as_ref());
    assert_eq!(rows.len(), 4, "{rows:?}");
    assert_eq!(
        (
            rows[0]["code"].as_str(),
            rows[0]["function"].as_str(),
            rows[0]["preamble"].as_str()
        ),
        ("954853", "1", "3")
    );
    assert_eq!(rows[0]["call"], "single call");
    for (r, f) in rows[1..].iter().zip(["2", "3", "4"]) {
        assert_eq!((r["code"].as_str(), r["function"].as_str()), ("123456", f));
        assert_eq!(r["kind"], "Tone page");
    }
    assert_eq!(rows[3]["words"], "both words inverted");
}

#[test]
fn a_batch_after_an_inverted_preamble() {
    // Four codes with the preamble index 4: (I + G0) mod 10 = 4.
    let pages = vec![
        TxPage::new(123_456, TxKind::Tone, ""),
        TxPage::new(123_457, TxKind::Numeric, "911"),
        TxPage::new(222_458, TxKind::Alpha, "GO NOW"),
        TxPage::new(341_459, TxKind::Tone, ""),
    ];
    let o = GscOptions {
        pages,
        batch: true,
        ..GscOptions::default()
    };
    let (rx, _) = receive(&signal(250e3, 0.0, &o), 250e3, 0.0);
    let rows = page_rows(rx.as_ref());
    assert_eq!(rows.len(), 4, "{rows:?}");
    for r in &rows {
        assert_eq!(r["call"], "batch", "{r:?}");
        assert_eq!(r["preamble"], "4", "{r:?}");
    }
    let codes: Vec<&str> = rows.iter().map(|r| r["code"].as_str()).collect();
    assert_eq!(codes, ["123456", "123457", "222458", "341459"]);
    assert_eq!(rows[1]["text"], "911");
    assert_eq!(rows[2]["text"], "GO NOW");
}

#[test]
fn calls_without_a_preamble_are_read() {
    let o = GscOptions {
        preamble: false,
        ..GscOptions::default()
    };
    let (rx, _) = receive(&signal(250e3, 0.0, &o), 250e3, 0.0);
    let rows = page_rows(rx.as_ref());
    let kinds: Vec<&str> = rows.iter().map(|r| r["kind"].as_str()).collect();
    assert_eq!(
        kinds,
        [
            "Tone page",
            "Numeric page",
            "Alpha page",
            "Voice page",
            "Voice end"
        ],
        "{rows:?}"
    );
    assert_eq!(rows[0]["code"], "N54853");
    assert_eq!(rows[0]["call"], "no preamble seen");
    assert_eq!(rows[1]["text"], "5551234");
}

/// Frequency modulation of the levels `level(t)` (Hz) at `rate` for `seconds`.
fn fm(rate: f64, seconds: f64, mut level: impl FnMut(f64) -> f64) -> Vec<Complex32> {
    let mut phase = 0.0f64;
    (0..(rate * seconds) as usize)
        .map(|k| {
            phase = (phase + TAU * level(k as f64 / rate) / rate) % TAU;
            Complex32::new(phase.cos() as f32, phase.sin() as f32)
        })
        .collect()
}

#[test]
fn signals_that_are_not_gsc_give_no_page() {
    let rate = 96e3;
    let mut signals: Vec<(String, Vec<Complex32>)> = Vec::new();
    for seed in 1..=3u64 {
        signals.push((
            format!("noise {seed}"),
            Awgn::with_sigma(0.1, seed).noise((rate * 40.0) as usize),
        ));
    }
    signals.push(("carrier".into(), fm(rate, 20.0, |_| 0.0)));
    for baud in [1200.0, 600.0, 300.0] {
        let mut x = 0x2545_F491u32;
        let mut bit = 0.0;
        let mut at = -1i64;
        signals.push((
            format!("2-FSK {baud}"),
            fm(rate, 30.0, |t| {
                let k = (t * baud) as i64;
                if k != at {
                    at = k;
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    bit = if x & 1 != 0 { 4500.0 } else { -4500.0 };
                }
                bit
            }),
        ));
    }
    signals.push((
        "voice".into(),
        fm(rate, 20.0, |t| {
            3000.0
                * (0.6 + 0.4 * (TAU * 3.0 * t).sin())
                * (0.5 * (TAU * 470.0 * t).sin() + 0.4 * (TAU * 1130.0 * t).sin())
        }),
    ));
    for (what, mut x) in signals {
        Awgn::new(1.0, 20.0, 12.5e3, rate, 9).process(&mut x);
        let (rx, fed) = receive(&x, rate, 0.0);
        assert!(
            page_rows(rx.as_ref()).is_empty(),
            "{what}: {:?}",
            page_rows(rx.as_ref())
        );
        assert!(fed.events.is_empty(), "{what}");
    }
}

#[test]
fn noise_sweep() {
    let rate = 96e3;
    let mut pages = Vec::new();
    for k in 0..3u32 {
        pages.push(TxPage::new(123_456 + k, TxKind::Numeric, "5551234"));
        pages.push(TxPage::new(213_456 + k, TxKind::Alpha, "CALL HQ"));
        pages.push(TxPage::new(954_853 + k, TxKind::Tone, ""));
    }
    let o = GscOptions {
        pages,
        offset_hz: 800.0,
        clock_ppm: 300.0,
        ..GscOptions::default()
    };
    let clean = signal(rate, 0.0, &o);
    let mut lines = Vec::new();
    for cn in [16.0, 10.0, 6.0, 3.0, 1.0, -1.0] {
        let (mut messages, mut tones) = (0, 0);
        for seed in 1..=2u64 {
            let mut x = clean.clone();
            Awgn::new(1.0, cn, 12.5e3, rate, seed).process(&mut x);
            let (rx, _) = receive(&x, rate, 0.0);
            for r in page_rows(rx.as_ref()) {
                match r["kind"].as_str() {
                    "Numeric page" if r["text"] == "5551234" => messages += 1,
                    "Alpha page" if r["text"] == "CALL HQ" => messages += 1,
                    "Tone page" if r["code"].starts_with("95485") => tones += 1,
                    _ => {}
                }
            }
        }
        lines.push(format!(
            "C/N {cn} dB in 12.5 kHz: messages {messages} of 12, tone pages {tones} of 6"
        ));
        if cn >= 16.0 {
            assert!(messages >= 11 && tones >= 6, "{lines:?}");
        }
    }
    println!("{}", lines.join("\n"));
}

#[test]
fn the_test_signal_locks_with_the_default_settings() {
    let rate = GSC.tuning.rate_hz;
    let v = sdrkit_dev_test::Harness::new(&GSC).values().unwrap();
    let mut g = (GSC.make_generator.unwrap())(&sdrkit_core::GenConfig::new(rate, v)).unwrap();
    let mut x = vec![Complex32::new(0.0, 0.0); (rate * 6.0) as usize];
    g.generate(&mut x);
    let mut rx = make(&GSC, &[]);
    configure(rx.as_mut(), rate, DIAL + 50e3, -50e3);
    let fed = feed(rx.as_mut(), &x, 8192);
    let r = results(rx.as_ref());
    assert!(num(r, "pages") >= 2.0, "{}", num(r, "pages"));
    assert_eq!(r.status().lock_state, LockState::Locked);
    check_events(
        &kinds(&sdrkit_stack_trunk::gsc::declaration()),
        &fed.events,
        true,
    );
    assert!(text(r, "note").contains("Not confirmed by a public document"));
}

#[test]
fn a_rate_below_the_least_is_refused() {
    let mut rx = make(&GSC, &[]);
    assert!(rx.configure(&InputInfo::new(48e3, DIAL, DIAL)).is_err());
}

#[test]
fn retune_keeps_clear_empties_and_the_state_round_trips() {
    let rate = 250e3;
    let o = GscOptions {
        pages: vec![TxPage::new(954_853, TxKind::Tone, "")],
        ..GscOptions::default()
    };
    let (mut rx, _) = receive(&signal(rate, 0.0, &o), rate, 0.0);
    assert_eq!(page_rows(rx.as_ref()).len(), 1);
    let saved = rx.save_state().expect("a saved state");
    rx.retuned(&InputInfo::new(rate, DIAL + 25e3, DIAL + 25e3).recording());
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert_eq!(page_rows(rx.as_ref()).len(), 1);
    invoke(rx.as_mut(), "clear");
    feed(rx.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert!(page_rows(rx.as_ref()).is_empty());
    assert_eq!(num(results(rx.as_ref()), "pages"), 0.0);
    let mut back = make(&GSC, &[]);
    configure(back.as_mut(), rate, DIAL, 0.0);
    back.restore_state(&saved).expect("the state is taken");
    feed(back.as_mut(), &vec![Complex32::new(0.0, 0.0); 25_000], 8192);
    assert_eq!(page_rows(back.as_ref()).len(), 1);
}
