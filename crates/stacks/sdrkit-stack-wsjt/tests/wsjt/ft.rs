//! The cases of FT8, FT4 and FT2.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::publish::keys;
use sdrkit_core::{Choice, Complex32, Generator, Receiver, Value};
use sdrkit_stack_wsjt::generator::{stations, WsjtGenerator};
use sdrkit_stack_wsjt::typed::DECODE;
use sdrkit_stack_wsjt::{Mode, Sub};

const SIX: [&str; 6] = [
    "CQ K1ABC FN42",
    "K1ABC W9XYZ -12",
    "W9XYZ K1ABC R-05",
    "CQ DX G4ABC IO91",
    "G4ABC DL1XYZ RR73",
    "CQ JA1XYZ PM95",
];

fn six(snr: f64) -> Vec<St<'static>> {
    let hz = [500.0, 950.0, 1400.0, 1850.0, 2300.0, 2800.0];
    let dt = [-0.6, 0.8, 0.1, -0.2, 0.5, 0.0];
    (0..6).map(|i| (SIX[i], hz[i], snr, dt[i], false)).collect()
}

#[test]
fn t_ft8_1_generator_stations_clean() {
    let sub = Sub::new(Mode::Ft8);
    let st: Vec<St> = stations(Mode::Ft8)
        .iter()
        .map(|s| (s.0, s.1, -6.0 + s.2, s.3, false))
        .collect();
    let a = audio_of(sub, &st, 30.0, true, 1);
    let d = decode_all(sub, &a);
    assert_eq!(
        d.len(),
        16,
        "{:?}",
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    for x in &d {
        let s = st.iter().find(|s| s.0 == x.text).unwrap();
        assert!(
            (x.hz - s.1).abs() < 1.0 && (x.dt - s.3).abs() < 0.05,
            "{} {} {}",
            x.text,
            x.hz,
            x.dt
        );
    }
    let cq = d.iter().find(|x| x.text == "CQ K1ABC FN42").unwrap();
    assert!(cq.cq && cq.call == "K1ABC" && cq.grid == "FN42");
}

#[test]
fn t_ft8_2_six_stations_at_minus_15() {
    let sub = Sub::new(Mode::Ft8);
    let st = six(-15.0);
    let d = decode_all(sub, &audio_of(sub, &st, 45.0, true, 2));
    let (ok, bad) = score(&d, &SIX);
    assert!(ok >= 17 && bad == 0, "{ok} decoded, {bad} false");
    let err: f64 = d.iter().map(|x| (x.snr as f64 + 15.0).abs()).sum::<f64>() / d.len() as f64;
    assert!(err < 2.5, "SNR error {err}");
}

/// T-FT8-3, T-FT8-4 and T-FT8-11: the generator through the whole receiver as IQ at 48 kHz, with a dial error, a clock error and
/// noise; the stations at their shifted frequency.
#[test]
fn t_ft8_3_4_dial_and_clock_errors_through_the_receiver() {
    for (dial, ppm, rate) in [(100.0, 100.0, 48000.0), (-100.0, -100.0, 48000.0)] {
        let mut g =
            WsjtGenerator::new(Sub::new(Mode::Ft8), rate, false)
                .unwrap();
        let mut iq = vec![Complex32::default(); (rate * 31.0) as usize];
        g.generate(&mut iq);
        shift(&mut iq, rate, dial);
        let mut skewed = clock_error(&iq, ppm);
        add_noise(&mut skewed, 10.0, rate, 3);
        let mut rx = receiver(Some(Mode::Ft8), &[], rate, 0.0);
        let ev = feed(&mut rx, &skewed, 4800);
        let got = texts(&ev);
        for s in stations(Mode::Ft8) {
            assert!(
                got.iter().filter(|t| *t == s.0).count() >= 2,
                "{dial} Hz {ppm} ppm: {} missing in {got:?}",
                s.0
            );
        }
        for e in ev.iter().filter(|e| e.is(&DECODE)) {
            let t = e.get(&keys::TEXT).unwrap();
            let s = stations(Mode::Ft8).iter().find(|s| s.0 == t).unwrap();
            let hz = num(e, "audio_hz");
            assert!((hz - (s.1 + dial)).abs() < 1.5, "{t}: {hz}");
            assert!((num(e, "dt_s") - s.3).abs() < 0.1, "{t} dt");
        }
    }
}

#[test]
fn t_ft8_5_late_stations_and_the_clock_warning() {
    let sub = Sub::new(Mode::Ft8);
    let st = [
        ("CQ K1ABC FN42", 700.0, -12.0, 1.5, false),
        ("K1ABC W9XYZ -12", 1300.0, -12.0, -1.5, false),
        ("CQ DX G4ABC IO91", 2000.0, -12.0, 1.4, false),
    ];
    let a = audio_of(sub, &st, 45.0, true, 5);
    let mut rx = receiver(Some(Mode::Ft8), &[], 8000.0, 0.0);
    let ev = feed(&mut rx, &iq_of(&a), 800);
    assert_eq!(texts(&ev).len(), 9, "{:?}", texts(&ev));
    let r = rx.results();
    assert_eq!(field(r, "clock_warning"), Some(&Value::Bool(true)));
    let e = field_f64(r, "clock_error_s");
    assert!((e - 1.4).abs() <= 0.1, "{e}");
}

#[test]
fn t_ft8_6_a_weak_station_beside_a_strong_one() {
    let sub = Sub::new(Mode::Ft8);
    let st = [
        ("CQ K1ABC FN42", 1000.0, -2.0, 0.0, false),
        ("K1ABC W9XYZ -12", 1015.0, -14.0, 0.2, false),
    ];
    let d = decode_all(sub, &audio_of(sub, &st, 30.0, true, 6));
    assert_eq!(
        d.len(),
        4,
        "{:?}",
        d.iter().map(|x| &x.text).collect::<Vec<_>>()
    );
    assert!(d
        .iter()
        .filter(|x| x.text == "K1ABC W9XYZ -12")
        .all(|x| x.pass >= 1));
}

#[test]
fn t_ft8_7_mirrored_stations() {
    let sub = Sub::new(Mode::Ft8);
    let st: Vec<St> = six(-15.0)
        .into_iter()
        .map(|s| (s.0, s.1, s.2, s.3, true))
        .collect();
    let d = decode_all(sub, &audio_of(sub, &st, 30.0, true, 7));
    let (ok, bad) = score(&d, &SIX);
    assert!(ok >= 11 && bad == 0, "{ok} {bad}");
    assert!(d.iter().all(|x| x.mirrored));
}

#[test]
fn t_ft8_8_audio_starting_7_s_into_a_slot() {
    let sub = Sub::new(Mode::Ft8);
    let st = [
        ("CQ K1ABC FN42", 800.0, -10.0, 0.0, false),
        ("K1ABC W9XYZ -12", 1600.0, -10.0, 0.0, false),
    ];
    let a = audio_of(sub, &st, 37.0, true, 8);
    let iq = iq_of(&a[7 * 8000..]);
    let settings = vec![("time_start", Value::Text("2026-01-01T00:00:07Z".into()))];
    let mut rx = receiver(Some(Mode::Ft8), &settings, 8000.0, 0.0);
    let ev = feed(&mut rx, &iq, 800);
    let dec: Vec<_> = ev.iter().filter(|e| e.is(&DECODE)).collect();
    assert_eq!(dec.len(), 2, "{:?}", texts(&ev));
    for e in dec {
        assert!(e.time >= 1_767_225_615.0);
        assert!(num(e, "dt_s").abs() < 0.06);
    }
}

#[test]
fn t_ft8_9_noise_carrier_and_nan_give_nothing() {
    let sub = Sub::new(Mode::Ft8);
    let mut a = audio_of(sub, &[], 30.0, true, 9);
    for (i, x) in a.iter_mut().enumerate() {
        *x += 0.1 * (2.0 * std::f32::consts::PI * 1234.0 * i as f32 / 8000.0).sin();
    }
    for x in a[50_000..50_400].iter_mut() {
        *x = f32::NAN;
    }
    assert!(decode_all(sub, &a).is_empty());
    let mut rx = receiver(Some(Mode::Ft8), &[], 8000.0, 0.0);
    assert!(texts(&feed(&mut rx, &iq_of(&a), 800)).is_empty());
}

#[test]
fn t_ft8_10_slot_search() {
    let sub = Sub::new(Mode::Ft8);
    let st = [
        ("CQ K1ABC FN42", 800.0, -8.0, 0.0, false),
        ("K1ABC W9XYZ -12", 1600.0, -8.0, 0.0, false),
    ];
    let a = audio_of(sub, &st, 79.2, true, 10);
    let iq = iq_of(&a[(4.2 * 8000.0) as usize..]);
    let mut rx = receiver(
        Some(Mode::Ft8),
        &[(
            "time_source",
            Value::Choice(sdrkit_stack_wsjt::slots::TimeChoice::Search.index()),
        )],
        8000.0,
        0.0,
    );
    let ev = feed(&mut rx, &iq, 800);
    assert_eq!(
        choice_key(rx.results(), "time_source"),
        "found_from_signals"
    );
    let near = ev
        .iter()
        .filter(|e| e.is(&DECODE) && num(e, "dt_s").abs() < 0.15)
        .count();
    assert!(near >= 6, "{near}: {:?}", texts(&ev));
}

#[test]
fn t_ft8_11_12_generator_at_384_ksps_and_swapped_iq() {
    let rate = 384_000.0;
    let mut g =
        WsjtGenerator::new(Sub::new(Mode::Ft8), rate, false).unwrap();
    let mut iq = vec![Complex32::default(); (rate * 16.0) as usize];
    g.generate(&mut iq);
    // The dial 20 kHz below the centre.
    shift(&mut iq, rate, -20_000.0);
    add_noise(&mut iq, 15.0, rate, 11);
    let mut rx = receiver(Some(Mode::Ft8), &[], rate, -20_000.0);
    let got = texts(&feed(&mut rx, &iq, 38_400));
    for s in stations(Mode::Ft8) {
        assert!(got.contains(&s.0.to_string()), "{} missing", s.0);
    }
    // I and Q swapped: the sideband lands on the far side.
    let swapped: Vec<Complex32> = iq.iter().map(|c| Complex32::new(c.im, c.re)).collect();
    let mut rx = receiver(Some(Mode::Ft8), &[], rate, -20_000.0);
    assert!(texts(&feed(&mut rx, &swapped, 38_400)).is_empty());
}

#[test]
fn t_ft8_13_threshold() {
    let sub = Sub::new(Mode::Ft8);
    let msgs = [
        "CQ K1ABC FN42",
        "K1ABC W9XYZ -12",
        "W9XYZ K1ABC R-05",
        "CQ DX G4ABC IO91",
        "G4ABC DL1XYZ RR73",
        "CQ JA1XYZ PM95",
        "TNX BOB 73 GL",
        "CQ PJ4/K1ABC",
    ];
    let mut r = rng(13);
    let st: Vec<St> = msgs
        .iter()
        .enumerate()
        .map(|(i, m)| {
            (
                *m,
                400.0 + 320.0 * i as f64,
                -21.0,
                r.uniform() * 2.0 - 1.0,
                false,
            )
        })
        .collect();
    let d = decode_all(sub, &audio_of(sub, &st, 45.0, true, 13));
    let (ok, bad) = score(&d, &msgs);
    assert!(
        ok as f64 >= 0.35 * 24.0 && bad == 0,
        "{ok} of 24, {bad} false"
    );
}

#[test]
fn t_ft4_1_to_4() {
    for (mode, snr, msgs) in [(Mode::Ft4, -12.0, 5), (Mode::Ft2, -8.0, 3)] {
        let sub = Sub::new(mode);
        let list = stations(mode);
        // T-FT4-1: the generator stations, 3 slots.
        let st: Vec<St> = list
            .iter()
            .map(|s| (s.0, s.1, -4.0 + s.2, s.3, false))
            .collect();
        let d = decode_all(sub, &audio_of(sub, &st, 3.0 * sub.period, true, 20));
        assert_eq!(
            d.len(),
            3 * msgs,
            "{mode:?} {:?}",
            d.iter().map(|x| &x.text).collect::<Vec<_>>()
        );
        // T-FT4-2: at the mode's level, all but at most one, SNR within 2.5 dB.
        let st: Vec<St> = list.iter().map(|s| (s.0, s.1, snr, s.3, false)).collect();
        let d = decode_all(sub, &audio_of(sub, &st, 3.0 * sub.period, true, 21));
        let texts: Vec<&str> = list.iter().map(|s| s.0).collect();
        let (ok, bad) = score(&d, &texts);
        assert!(ok + 1 >= 3 * msgs && bad == 0, "{mode:?}: {ok} {bad}");
        let err: f64 = d.iter().map(|x| (x.snr as f64 - snr).abs()).sum::<f64>() / d.len() as f64;
        assert!(err < 2.5, "{mode:?} SNR error {err}");
        // T-FT4-3: mirrored and late.
        let dtmax = if mode == Mode::Ft2 { 0.8 } else { 1.5 };
        let st: Vec<St> = list
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    s.0,
                    s.1,
                    snr + 2.0,
                    if i % 2 == 0 { dtmax } else { -dtmax },
                    true,
                )
            })
            .collect();
        let d = decode_all(sub, &audio_of(sub, &st, 2.0 * sub.period, true, 22));
        let (ok, bad) = score(&d, &texts);
        assert!(
            ok + 1 >= 2 * msgs && bad == 0 && d.iter().all(|x| x.mirrored),
            "{mode:?} mirrored: {ok} {bad}"
        );
        // T-FT4-4: noise alone.
        assert!(decode_all(sub, &audio_of(sub, &[], 4.0 * sub.period, true, 23)).is_empty());
    }
}

#[test]
fn t_ft4_5_threshold() {
    for (mode, snr, share) in [(Mode::Ft4, -16.0, 0.45), (Mode::Ft2, -13.0, 0.30)] {
        let sub = Sub::new(mode);
        let msgs = [
            "CQ K1ABC FN42",
            "K1ABC W9XYZ -12",
            "W9XYZ K1ABC R-05",
            "CQ DX G4ABC IO91",
            "G4ABC DL1XYZ RR73",
            "CQ JA1XYZ PM95",
            "TNX BOB 73 GL",
            "CQ PJ4/K1ABC",
        ];
        let st: Vec<St> = msgs
            .iter()
            .enumerate()
            .map(|(i, m)| (*m, 400.0 + 320.0 * i as f64, snr, 0.0, false))
            .collect();
        let d = decode_all(sub, &audio_of(sub, &st, 3.0 * sub.period, true, 24));
        let (ok, bad) = score(&d, &msgs);
        assert!(
            ok as f64 >= share * 24.0 && bad == 0,
            "{mode:?}: {ok} of 24, {bad} false"
        );
    }
}

#[test]
fn listen_does_not_change_the_decodes() {
    let sub = Sub::new(Mode::Ft4);
    let st: Vec<St> = stations(Mode::Ft4)
        .iter()
        .map(|s| (s.0, s.1, -6.0 + s.2, s.3, false))
        .collect();
    let iq = iq_of(&audio_of(sub, &st, 15.0, true, 30));
    let mut a = receiver(None, &[("listen", Value::Bool(false))], 8000.0, 0.0);
    let mut b = receiver(None, &[("listen", Value::Bool(true))], 8000.0, 0.0);
    let mut out = sdrkit_core::Output::default();
    let mut heard = 0;
    let (mut ea, mut eb) = (Vec::new(), Vec::new());
    for c in iq.chunks(800) {
        a.feed(c, &mut out);
        ea.append(&mut out.events);
        out.clear();
        b.feed(c, &mut out);
        heard += out
            .audio_runs()
            .map(|p| p.samples.len() / usize::from(p.channels.max(1)))
            .sum::<usize>();
        eb.append(&mut out.events);
        out.clear();
    }
    let mut ta = texts(&ea);
    let mut tb = texts(&eb);
    ta.sort();
    tb.sort();
    assert_eq!(ta, tb);
    assert!(!ta.is_empty());
    assert!(
        (heard as f64 - 15.0 * 8000.0).abs() < 15.0 * 400.0,
        "{heard}"
    );
}

#[test]
fn t_ft8_14_crowded_band() {
    // 100 stations from -20 to 0 dB, overlapping in frequency, one slot.
    let sub = Sub::new(Mode::Ft8);
    let mut r = rng(14);
    let calls: Vec<String> = (0..100)
        .map(|i| {
            format!(
                "K{}{}{}",
                i % 10,
                (b'A' + (i / 10) as u8) as char,
                (b'A' + (i % 7) as u8) as char
            )
        })
        .collect();
    let texts: Vec<String> = (0..100)
        .map(|i| {
            format!(
                "{} W{}XYZ {}",
                calls[i],
                i % 10,
                ["RR73", "73", "RRR", "-10", "+05"][i % 5]
            )
        })
        .collect();
    let st: Vec<St> = (0..100)
        .map(|i| {
            (
                texts[i].as_str(),
                200.0 + 27.5 * i as f64 + r.uniform() * 10.0,
                -20.0 + 20.0 * r.uniform(),
                r.uniform() - 0.5,
                false,
            )
        })
        .collect();
    let d = decode_all(sub, &audio_of(sub, &st, 15.0, true, 140));
    let sent: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let (ok, bad) = score(&d, &sent);
    assert!(ok >= 93 && bad == 0, "{ok} of 100, {bad} false");
}
