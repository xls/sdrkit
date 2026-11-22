//! The cases of WSPR, JT65, JT9, JT4, Q65, FST4, FST4W and MSK144: generator stations, the decode targets at their levels,
//! impairments, and noise alone.

use super::common::*;
use sdrkit_core::{Choice, Complex32, Generator, LockState};
use sdrkit_stack_wsjt::generator::{stations, WsjtGenerator};
use sdrkit_stack_wsjt::runner::DecodeState;
use sdrkit_stack_wsjt::{Mode, Sub};

fn list(mode: Mode, offset_db: f64) -> Vec<St<'static>> {
    stations(mode)
        .iter()
        .map(|s| (s.0, s.1, offset_db + s.2, s.3, false))
        .collect()
}

fn all_decoded(sub: Sub, st: &[St], slots: f64, seed: u64) -> Vec<sdrkit_stack_wsjt::rx::Decode> {
    let a = audio_of(sub, st, slots * sub.period, true, seed);
    let d = decode_with(sub, &a, DecodeState::default());
    let sent: Vec<&str> = st.iter().map(|s| s.0).collect();
    let (ok, bad) = score(&d, &sent);
    assert_eq!(
        bad,
        0,
        "{}: false decodes in {:?}",
        sub.label(),
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    assert!(
        ok >= st.len() * slots as usize,
        "{}: {ok} of {} in {:?}",
        sub.label(),
        st.len() * slots as usize,
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    d
}

#[test]
fn t_ws_1_2_5_wspr() {
    let sub = Sub::new(Mode::Wspr);
    let d = all_decoded(sub, &list(Mode::Wspr, -10.0), 1.0, 40);
    for x in &d {
        let s = stations(Mode::Wspr).iter().find(|s| s.0 == x.text).unwrap();
        assert!(
            (x.hz - s.1).abs() < 0.3 && (x.dt - s.3).abs() < 0.1,
            "{} {} {}",
            x.text,
            x.hz,
            x.dt
        );
    }
    let k = d.iter().find(|x| x.text == "K1ABC FN42 37").unwrap();
    assert_eq!(
        (k.dbm, k.call.as_str(), k.grid.as_str()),
        (37, "K1ABC", "FN42")
    );
    assert!(d.iter().any(|x| x.text == "<K1ABC> FN42AX 37"));
    // T-WS-2: four stations at -26 dB.
    let st = [
        ("K1ABC FN42 37", 1420.0, -26.0, 0.0, false),
        ("G4XYZ IO91 23", 1460.0, -26.0, 1.5, false),
        ("DL1XYZ/P 10", 1530.0, -26.0, -1.5, false),
        ("PJ4/K1ABC 30", 1570.0, -26.0, 0.7, false),
    ];
    let d = all_decoded(sub, &st, 1.0, 41);
    assert!(d.len() <= 4);
    let err: f64 = d.iter().map(|x| (x.snr as f64 + 26.0).abs()).sum::<f64>() / d.len() as f64;
    assert!(err < 2.5, "{err}");
    // T-WS-5: noise alone.
    assert!(decode_all(sub, &audio_of(sub, &[], 120.0, true, 42)).is_empty());
}

#[test]
fn t_ws_3_wspr_impairments_and_mirrored() {
    let sub = Sub::new(Mode::Wspr);
    let st = [
        ("K1ABC FN42 37", 1420.0, -24.0, 0.0, true),
        ("G4XYZ IO91 23", 1460.0, -24.0, 1.0, true),
        ("DL1XYZ/P 10", 1530.0, -24.0, -1.0, true),
        ("PJ4/K1ABC 30", 1570.0, -24.0, 0.5, true),
    ];
    let d = decode_all(sub, &audio_of(sub, &st, 120.0, true, 43));
    assert!(
        d.iter().filter(|x| x.mirrored).count() >= 3,
        "{:?}",
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    // A dial error and a clock error, through the receiver.
    let rate = 12000.0;
    let mut g = WsjtGenerator::new(sub, rate, false).unwrap();
    let mut iq = vec![Complex32::default(); (rate * 121.0) as usize];
    g.generate(&mut iq);
    shift(&mut iq, rate, 100.0);
    let mut sk = clock_error(&iq, 100.0);
    add_noise(&mut sk, 0.0, rate, 44);
    let mut rx = receiver(Some(Mode::Wspr), &[], rate, 0.0);
    let got = texts(&feed(&mut rx, &sk, 12000));
    assert!(got.len() >= 3, "{got:?}");
}

#[test]
fn t_ws_4_wspr_slot_search() {
    let sub = Sub::new(Mode::Wspr);
    let st = [
        ("K1ABC FN42 37", 1450.0, -18.0, 0.0, false),
        ("G4XYZ IO91 23", 1500.0, -18.0, 0.6, false),
    ];
    let a = audio_of(sub, &st, 360.0, true, 45);
    let iq = iq_of(&a[47 * 8000..]);
    let mut rx = receiver(
        Some(Mode::Wspr),
        &[(
            "time_source",
            sdrkit_core::Value::Choice(sdrkit_stack_wsjt::slots::TimeChoice::Search.index()),
        )],
        8000.0,
        0.0,
    );
    let ev = feed(&mut rx, &iq, 8000);
    let got = texts(&ev);
    assert!(
        got.contains(&"K1ABC FN42 37".to_string()) && got.contains(&"G4XYZ IO91 23".to_string()),
        "{got:?}"
    );
    assert_eq!(
        choice_key(sdrkit_core::owner::Owner::results(&rx), "time_source"),
        "found_from_signals"
    );
}

#[test]
fn jt65_targets() {
    for sm in 0..3u8 {
        let sub = Sub::new(Mode::Jt65).with_sub(sm);
        for snr in [10.0, 0.0, -18.0] {
            all_decoded(
                sub,
                &[("K1ABC W9XYZ -15", 1200.0, snr, 0.0, false)],
                1.0,
                50 + sm as u64,
            );
        }
    }
    let sub = Sub::new(Mode::Jt65);
    all_decoded(sub, &list(Mode::Jt65, -10.0), 1.0, 53);
    // Start offsets and frequencies.
    all_decoded(
        sub,
        &[
            ("CQ K1ABC FN42", 250.0, -19.0, -2.4, false),
            ("K1ABC W9XYZ -12", 2700.4, -19.0, 2.4, false),
        ],
        1.0,
        54,
    );
    // Three stations, two 90 Hz apart.
    all_decoded(
        sub,
        &[
            ("CQ K1ABC FN42", 1000.0, -14.0, 0.0, false),
            ("K1ABC W9XYZ -12", 1090.0, -16.0, 0.5, false),
            ("CQ DX G4ABC IO91", 2000.0, -18.0, 0.0, false),
        ],
        1.0,
        55,
    );
    assert!(decode_all(sub, &audio_of(sub, &[], 60.0, true, 56)).is_empty());
}

#[test]
fn jt9_and_jt4_targets() {
    let sub = Sub::new(Mode::Jt9);
    all_decoded(sub, &list(Mode::Jt9, -14.0), 1.0, 60);
    all_decoded(
        sub,
        &[
            ("CQ K1ABC FN42", 600.0, -22.0, 0.0, false),
            ("K1ABC W9XYZ -12", 1400.0, -22.0, 1.0, false),
        ],
        1.0,
        61,
    );
    assert!(decode_all(sub, &audio_of(sub, &[], 60.0, true, 62)).is_empty());
    for sm in 0..7u8 {
        let sub = Sub::new(Mode::Jt4).with_sub(sm);
        all_decoded(
            sub,
            &[("CQ K1ABC FN42", 1000.0, -16.0, 0.0, false)],
            1.0,
            63 + sm as u64,
        );
    }
    let sub = Sub::new(Mode::Jt4);
    let d = all_decoded(
        sub,
        &[("CQ K1ABC FN42", 1000.0, -10.0, 0.0, false)],
        1.0,
        70,
    );
    assert!((d[0].hz - 1000.0).abs() < 2.5 && d[0].dt.abs() < 0.1 && d.len() == 1);
    all_decoded(
        sub,
        &[
            ("CQ K1ABC FN42", 800.0, -13.0, 0.0, false),
            ("K1ABC W9XYZ -12", 1500.0, -13.0, 0.4, false),
        ],
        1.0,
        71,
    );
    assert!(decode_all(sub, &audio_of(sub, &[], 60.0, true, 72)).is_empty());
}

#[test]
fn q65_on_noise_decodes_nothing_and_is_not_refused() {
    // The code of Q65 is built in: on noise the receiver decodes nothing, refuses nothing and states the unconfirmed parts.
    let a = noise(70 * 8000, SIGMA, 83);
    let mut rx = receiver(Some(Mode::Q65), &[], 8000.0, 0.0);
    assert!(texts(&feed(&mut rx, &iq_of(&a), 8000)).is_empty());
    let st = sdrkit_core::owner::Owner::results(&rx).status();
    assert_ne!(st.lock_state, LockState::Off, "{}", st.summary);
    assert!(st.summary.contains("no public document"), "{}", st.summary);
    assert!(!st.summary.contains("to decode"), "{}", st.summary);
}

#[test]
fn fst4_and_fst4w_targets() {
    for p in [15.0, 30.0, 60.0, 120.0] {
        let sub = Sub::new(Mode::Fst4).with_period(p);
        let d = all_decoded(
            sub,
            &[("CQ K1ABC FN42", 1000.0, -10.0, 0.0, false)],
            1.0,
            90,
        );
        assert!(
            d[0].dt.abs() < 0.1
                && (d[0].hz - 1000.0).abs() < 0.5
                && (d[0].snr as f64 + 10.0).abs() < 3.0,
            "{p}: {:?}",
            d[0]
        );
    }
    let sub = Sub::new(Mode::Fst4);
    all_decoded(sub, &list(Mode::Fst4, -12.0), 1.0, 91);
    assert!(decode_with(sub, &audio_of(sub, &[], 60.0, true, 93), DecodeState::default()).is_empty());
    let w = Sub::new(Mode::Fst4w);
    all_decoded(
        w,
        &[
            ("K1ABC FN42 37", 1400.0, -12.0, 0.0, false),
            ("G4XYZ IO91 23", 1600.0, -15.0, 0.3, false),
        ],
        1.0,
        94,
    );
    assert!(decode_with(w, &audio_of(w, &[], 120.0, true, 95), DecodeState::default()).is_empty());
}

#[test]
fn msk144_targets() {
    let sub = Sub::new(Mode::Msk144);
    // A continuous transmission at -6 dB, and noise alone.
    let d = decode_with(
        sub,
        &audio_of(sub, &list(Mode::Msk144, -6.0), 15.0, true, 100),
        DecodeState::default(),
    );
    assert_eq!(
        d.iter().map(|x| x.text.as_str()).collect::<Vec<_>>(),
        vec!["CQ K1ABC FN42"]
    );
    for k in 0..4 {
        assert!(
            decode_with(sub, &audio_of(sub, &[], 15.0, true, 101 + k), DecodeState::default()).is_empty()
        );
    }
    // Meteor bursts of 150 ms at +4 dB at several times and carriers; two stations in one period.
    let mut a = audio_of(sub, &[], 15.0, true, 105);
    for (text, fc, t) in [
        ("CQ K1ABC FN42", 1200.0, 3.0),
        ("G4XYZ DL1ABC JO62", 1900.0, 9.5),
    ] {
        let s = sdrkit_stack_wsjt::encode::channel_symbols(&sub, text).unwrap();
        let mut w = Vec::new();
        sdrkit_stack_wsjt::encode::audio_of(
            &sdrkit_stack_wsjt::encode::Transmission {
                sub,
                hz: fc,
                amp: amp(4.0),
                mirrored: false,
            },
            &s,
            8000.0,
            0.15,
            &mut w,
        );
        let st = (t * 8000.0) as usize;
        for (i, v) in w.iter().enumerate() {
            a[st + i] += v;
        }
    }
    let d = decode_with(sub, &a, DecodeState::default());
    assert_eq!(
        d.len(),
        2,
        "{:?}",
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    for x in &d {
        let (fc, t) = if x.text.starts_with("CQ") {
            (1200.0, 3.0)
        } else {
            (1900.0, 9.5)
        };
        assert!(
            (x.hz - (fc - 500.0)).abs() < 10.0 && (x.dt - t).abs() < 0.25,
            "{x:?}"
        );
    }
}

#[test]
fn the_null_72_bit_message_is_not_reported() {
    // "000AAA 000AAA RA90" packs to the all-zero word: every data tone is the lowest one, as an unmodulated tone produces.
    assert_eq!(
        sdrkit_stack_wsjt::proto::msg72::pack72("000AAA 000AAA RA90").ok(),
        Some([0u8; 12])
    );
    for mode in [Mode::Jt65, Mode::Jt4, Mode::Jt9] {
        let sub = Sub::new(mode);
        let st = [
            ("000AAA 000AAA RA90", 1500.0, -10.0, 0.0, false),
            ("CQ K1ABC FN42", 1000.0, -10.0, 0.0, false),
        ];
        let d = decode_all(sub, &audio_of(sub, &st, sub.period, true, 46));
        let t: Vec<&str> = d.iter().map(|x| x.text.as_str()).collect();
        assert!(t.contains(&"CQ K1ABC FN42"), "{mode:?}: {t:?}");
        assert!(!t.contains(&"000AAA 000AAA RA90"), "{mode:?}: {t:?}");
    }
}
