//! `fsk441`: the character codes, the reader on clean bursts at several offsets and rates, its pass marks in noise, a short ping
//! found in a long recording, and nothing from noise or a carrier.

use crate::common::noise;
use crate::scatter::*;
use sdrkit_stack_wsjt::scatter::fsk441::{self, read};
use sdrkit_stack_wsjt::FSK441;

const MSG: &str = "W9XYZ K1JT 26  ";

fn both_calls(t: &str) -> bool {
    t.contains("W9XYZ") && t.contains("K1JT")
}

#[test]
fn clean_bursts_are_read_at_each_offset() {
    for df in [0.0, -120.0, 85.0, 250.0] {
        let x = fsk441::audio(&MSG.repeat(3), 8000.0, df, 0.3);
        let mut a = vec![0f32; 12000];
        put(&mut a, &x, 3000);
        let r = read(&a, 8000.0, 300.0).unwrap_or_else(|| panic!("{df}: nothing read"));
        assert!(both_calls(&r.text), "{df}: {}", r.text);
        assert!((r.df_hz - df).abs() <= 20.0, "{df}: {}", r.df_hz);
    }
}

#[test]
fn other_audio_rates_read_k1jt() {
    for rate in [8000.0, 11025.0, 24000.0] {
        let x = fsk441::audio(&MSG.repeat(3), rate, 0.0, 0.3);
        let mut a = vec![0f32; (1.5 * rate) as usize];
        put(&mut a, &x, (0.3 * rate) as usize);
        let r = read(&a, rate, 300.0).unwrap_or_else(|| panic!("{rate}: nothing read"));
        assert!(r.text.contains("K1JT"), "{rate}: {}", r.text);
    }
}

/// Bursts of six repeats at +40 Hz in noise of `snr` dB in 2500 Hz, 10 seeds: how many give both calls.
fn passes(snr: f64) -> usize {
    let x = fsk441::audio(&MSG.repeat(6), 8000.0, 40.0, amp_at(snr, 8000.0));
    (0..10)
        .filter(|&seed| {
            let mut a = noise(x.len() + 1600, SIGMA, 100 + seed);
            put(&mut a, &x, 800);
            read(&a, 8000.0, 300.0).is_some_and(|r| both_calls(&r.text))
        })
        .count()
}

#[test]
fn pass_marks_in_noise() {
    assert_eq!(passes(20.0), 10);
    assert_eq!(passes(10.0), 10);
    let p4 = passes(4.0);
    for snr in [0.0, -4.0, -8.0] {
        eprintln!("FSK441 at {snr} dB: {} of 10", passes(snr));
    }
    assert!(p4 >= 8, "+4 dB: {p4} of 10");
}

#[test]
fn a_short_ping_in_a_long_recording_is_one_burst() {
    let x = fsk441::audio(&MSG.repeat(2), 8000.0, -60.0, 0.45);
    let x = &x[..(0.18 * 8000.0) as usize];
    let mut a = noise(12 * 8000, 0.12, 7);
    put(&mut a, x, 5 * 8000);
    let mut rx = receiver(&FSK441, &[]);
    let ev = feed_audio(rx.as_mut(), &a, 1500);
    let d = decodes(&ev);
    assert_eq!(
        d.len(),
        1,
        "{:?}",
        d.iter().map(|e| text(e)).collect::<Vec<_>>()
    );
    assert!(text(d[0]).contains("W9XYZ"), "{}", text(d[0]));
    let fields = rx.results().fields();
    assert_eq!(
        fields.at(&sdrkit_stack_wsjt::typed::Fsk441Fields::BURSTS),
        Some(1)
    );
    // The row's signal time is the start of the burst read.
    let t = rx
        .results()
        .table(rx.results().decl().table_id("decodes").unwrap())
        .unwrap()
        .rows_newest_first()
        .next()
        .map(|(_, r)| {
            r.at(&sdrkit_stack_wsjt::typed::Fsk441DecodeRow::TIME_S)
                .unwrap_or(0.0)
        })
        .unwrap();
    assert!((t - 5.0).abs() < 0.3, "{t}");
}

#[test]
fn noise_and_a_carrier_give_nothing() {
    let mut rx = receiver(&FSK441, &[]);
    let a = noise(60 * 8000, SIGMA, 11);
    assert!(decodes(&feed_audio(rx.as_mut(), &a, 4000)).is_empty());
    let mut rx = receiver(&FSK441, &[]);
    let c: Vec<f32> = (0..3 * 8000)
        .map(|i| 0.3 * (std::f64::consts::TAU * 1500.0 * i as f64 / 8000.0).cos() as f32)
        .collect();
    let a = in_noise(&c, 10.0, 4.0, 12);
    assert!(decodes(&feed_audio(rx.as_mut(), &a, 4000)).is_empty());
}

#[test]
fn the_test_signal_decodes_with_listen_on_and_off() {
    let mut texts = Vec::new();
    for listen in [false, true] {
        let mut g = (FSK441.make_generator.unwrap())(&sdrkit_core::GenConfig::new(
            48_000.0,
            sdrkit_core::Values::defaults(&(FSK441.settings)()),
        ))
        .unwrap();
        let mut iq = vec![sdrkit_core::Complex32::default(); 24 * 48_000];
        g.generate(&mut iq);
        let mut rx = receiver(&FSK441, &[("listen", sdrkit_core::Value::Bool(listen))]);
        rx.configure(&sdrkit_core::InputInfo::new(48_000.0, 50e6 + 20e3, 50e6).recording())
            .unwrap();
        let mut out = sdrkit_core::Output::default();
        let mut ev = Vec::new();
        for c in iq.chunks(4800) {
            rx.feed(c, &mut out);
            ev.append(&mut out.events);
            out.clear();
        }
        let t: Vec<String> = decodes(&ev).iter().map(|e| text(e).to_owned()).collect();
        assert_eq!(t.len(), 4, "{t:?}");
        assert!(t.iter().all(|x| x.contains("K0TST")), "{t:?}");
        texts.push(t);
    }
    assert_eq!(texts[0], texts[1]);
}

#[test]
fn clear_empties_the_decode_table() {
    let x = fsk441::audio(&MSG.repeat(3), 8000.0, 0.0, 0.3);
    let a = in_noise(&x, 6.0, 2.0, 21);
    let mut rx = receiver(&FSK441, &[]);
    let ev = feed_audio(rx.as_mut(), &a, 4000);
    assert_eq!(decodes(&ev).len(), 1);
    let rows = |rx: &dyn sdrkit_core::Receiver| {
        let r = rx.results();
        r.decl()
            .table_id("decodes")
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.rows_newest_first().count())
    };
    assert_eq!(rows(rx.as_ref()), 1);
    let clear = rx.commands().id("clear").unwrap();
    let args = rx.commands().def(clear).unwrap().new_args();
    sdrkit_core::owner::invoke(rx.as_mut(), clear, &args).unwrap();
    feed_audio(rx.as_mut(), &crate::common::noise(8000, SIGMA, 22), 4000);
    assert_eq!(rows(rx.as_ref()), 0);
    assert_eq!(
        rx.results()
            .fields()
            .at(&sdrkit_stack_wsjt::typed::ScatterFields::DECODES_TOTAL),
        Some(0)
    );
}

#[test]
fn a_change_of_frequency_keeps_the_decodes_and_forgets_the_bursts() {
    use sdrkit_stack_wsjt::typed::{shape, Fsk441Fields, ScatterFields};
    for id in ["fsk441", "jt6m", "iscat", "pi4"] {
        assert_eq!(
            shape(id).kept_keys(),
            Ok(vec!["table decodes".to_owned(), "decodes_total".to_owned()])
        );
    }
    let x = fsk441::audio(&MSG.repeat(3), 8000.0, 0.0, 0.3);
    let a = in_noise(&x, 6.0, 2.0, 21);
    let mut rx = receiver(&FSK441, &[]);
    let ev = feed_audio(rx.as_mut(), &a, 4000);
    assert_eq!(decodes(&ev).len(), 1);
    let rows = |rx: &dyn sdrkit_core::Receiver| {
        let r = rx.results();
        r.decl()
            .table_id("decodes")
            .and_then(|id| r.table(id))
            .map_or(0, |t| t.rows_newest_first().count())
    };
    assert_eq!(rx.results().fields().at(&Fsk441Fields::BURSTS), Some(1));
    // The engine tells the receiver of the new frequency, then tells it to forget.
    rx.retuned(&sdrkit_core::InputInfo::new(8000.0, 50.1e6, 50.1e6).recording());
    rx.forget();
    feed_audio(rx.as_mut(), &crate::common::noise(8000, SIGMA, 22), 4000);
    assert_eq!(rows(rx.as_ref()), 1);
    let f = rx.results().fields();
    assert_eq!(f.at(&ScatterFields::DECODES_TOTAL), Some(1));
    assert_eq!(f.at(&Fsk441Fields::BURSTS), Some(0));
}
