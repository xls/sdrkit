//! The JS8 cases, through the whole receiver with the JS8 generator.

use super::common::*;
use sdrkit_core::owner::Owner;
use sdrkit_core::publish::keys;
use sdrkit_core::{Choice, Complex32, Event, Generator, Receiver};
use sdrkit_graph::StackGraph;
use sdrkit_stack_wsjt::components::Js8Speed;
use sdrkit_stack_wsjt::generator::{js8_stations, Js8Station, WsjtGenerator};
use sdrkit_stack_wsjt::proto::js8msg::Outgoing;
use sdrkit_stack_wsjt::table::WATERFALL_SERIES;
use sdrkit_stack_wsjt::typed::{Js8FrameEvent, Js8MessageEvent, JS8_FRAME, JS8_MESSAGE};
use sdrkit_stack_wsjt::Mode;

/// The JS8 stations at their levels (the strongest at its SNR in 2500 Hz), moved by a tuning error of `tune` Hz.
fn signal(list: &[Js8Station], seconds: f64, tune: f64, seed: u64) -> Vec<Complex32> {
    let rate = 12000.0;
    let mut g = WsjtGenerator::js8(rate, list).unwrap();
    let mut iq = vec![Complex32::default(); (rate * seconds) as usize];
    g.generate(&mut iq);
    if tune != 0.0 {
        shift(&mut iq, rate, tune);
    }
    let top = list.iter().map(|s| s.snr).fold(-20.0, f64::max);
    add_noise(&mut iq, top, rate, seed);
    iq
}

fn run(list: &[Js8Station], seconds: f64, tune: f64) -> (Vec<Event>, StackGraph) {
    let rate = 12000.0;
    let iq = signal(list, seconds, tune, 7);
    let mut rx = receiver(Some(Mode::Js8), &[], rate, 0.0);
    let ev = feed(&mut rx, &iq, 1200);
    (ev, rx)
}

fn frames(ev: &[Event]) -> Vec<(String, f64, i64, u64)> {
    ev.iter()
        .filter(|e| e.is(&JS8_FRAME))
        .map(|e| {
            (
                e.get(&keys::TEXT).unwrap().to_string(),
                e.freq_hz,
                e.get(&keys::SNR_DB).unwrap() as i64,
                u64::from(e.at(&Js8FrameEvent::SUBMODE).unwrap().index()),
            )
        })
        .collect()
}

fn hb(call: &str, grid: &str, sub: u8, hz: f64, snr: f64, dt: f64) -> Js8Station {
    Js8Station {
        msg: Outgoing::Heartbeat {
            call: call.into(),
            grid: grid.into(),
            cq: None,
        },
        submode: sub,
        hz,
        snr,
        dt,
        drift: 0.0,
        first: 0,
        every: 1,
    }
}

#[test]
fn t_js8_1_the_generator_stations() {
    let (ev, rx) = run(&js8_stations(), 61.0, 0.0);
    let f = frames(&ev);
    let has = |t: &str| f.iter().any(|x| x.0 == t);
    for t in [
        "LY2ABC: @HB HEARTBEAT KO24",
        "DL1XYZ: @ALLCALL CQ CQ CQ JO62",
        "OK1XX: LY2ABC SNR -12",
        "YL2XYZ: @HB HEARTBEAT KO26",
        "ES1ABC: @ALLCALL CQ KO29",
    ] {
        assert!(has(t), "{t} missing from {f:?}");
    }
    let msgs: Vec<&Event> = ev.iter().filter(|e| e.is(&JS8_MESSAGE)).collect();
    assert!(
        msgs.iter().any(|m| m.get(&keys::TEXT)
            == Some("G4ABC: DL1XYZ HELLO FROM LONDON. HOW COPY?")
            && m.at(&Js8MessageEvent::COMPLETE) == Some(true)),
        "{:?}",
        msgs.iter().map(|m| m.get(&keys::TEXT)).collect::<Vec<_>>()
    );
    assert!(msgs.iter().any(|m| m
        .get(&keys::TEXT)
        .unwrap()
        .starts_with("EA3XYZ: OK1XX>SP5AB")));
    let res = rx.results();
    assert!(res.points().contains_key("LY2ABC") && res.points().contains_key("DL1XYZ"));
    let slots = |s: u64| -> std::collections::BTreeSet<i64> {
        ev.iter()
            .filter(|e| {
                e.is(&JS8_FRAME) && e.at(&Js8FrameEvent::SUBMODE) == Js8Speed::from_index(s as u16)
            })
            .map(|e| e.time as i64)
            .collect()
    };
    assert!(
        slots(0).len() >= 4 && slots(1).len() >= 5 && slots(2).len() >= 9,
        "{} {} {}",
        slots(0).len(),
        slots(1).len(),
        slots(2).len()
    );
    let g4 = f.iter().find(|x| x.0.starts_with("G4ABC")).unwrap();
    assert!((g4.2 + 4).abs() <= 3, "{}", g4.2);
}

#[test]
fn t_js8_2_tuning_error_with_drift() {
    let mut s = hb("LY2ABC", "KO24", 0, 600.0, -8.0, 0.0);
    s.drift = 1.0;
    let (ev, _) = run(&[s], 31.0, 245.0);
    let f = frames(&ev);
    assert!(!f.is_empty());
    for x in &f {
        assert!((x.1 - 14e6 - 845.0).abs() <= 3.0, "{}", x.1 - 14e6);
    }
}

#[test]
fn t_js8_3_to_6_offsets_weak_and_crowded() {
    // Starts at DT +2.0 and -1.2 s; a weak heartbeat at -17 dB; Fast at +1.0 and Turbo at -0.5.
    let list = [
        hb("LY2ABC", "KO24", 0, 600.0, -8.0, 2.0),
        hb("DL1XYZ", "JO62", 0, 1200.0, -8.0, -1.2),
        hb("OK1XX", "JO70", 0, 1800.0, -17.0, 0.0),
        hb("YL2XYZ", "KO26", 1, 2400.0, -6.0, 1.0),
        hb("ES1ABC", "KO29", 2, 2900.0, -4.0, -0.5),
    ];
    let (ev, _) = run(&list, 31.0, 0.0);
    let f = frames(&ev);
    for c in ["LY2ABC", "DL1XYZ", "OK1XX", "YL2XYZ", "ES1ABC"] {
        assert!(
            f.iter().any(|x| x.0.starts_with(c)),
            "{c} missing from {f:?}"
        );
    }
    for e in ev.iter().filter(|e| e.is(&JS8_FRAME)) {
        let _ = e;
    }
    // Five signals in one slot close in frequency.
    let list: Vec<Js8Station> = (0..5)
        .map(|i| {
            hb(
                ["LY2ABC", "DL1XYZ", "OK1XX", "G4ABC", "SP5AB"][i],
                "KO24",
                0,
                1000.0 + 65.0 * i as f64,
                -8.0,
                0.1 * i as f64,
            )
        })
        .collect();
    let (ev, _) = run(&list, 16.0, 0.0);
    assert_eq!(frames(&ev).len(), 5, "{:?}", frames(&ev));
}

#[test]
fn t_js8_8_noise_only() {
    let (ev, rx) = run(&[], 31.0, 0.0);
    assert!(frames(&ev).is_empty());
    assert!(field(rx.results(), "utc").is_some());
}

#[test]
fn t_js8_7_listen_and_the_waterfall() {
    let rate = 12000.0;
    let iq = signal(&js8_stations()[..1], 16.0, 0.0, 9);
    let mut a = receiver(Some(Mode::Js8), &[], rate, 0.0);
    let mut b = receiver(
        Some(Mode::Js8),
        &[("listen", sdrkit_core::Value::Bool(true))],
        rate,
        0.0,
    );
    let ea = feed(&mut a, &iq, 1200);
    let mut out = sdrkit_core::Output::default();
    let mut heard = 0;
    let mut eb = Vec::new();
    for c in iq.chunks(1200) {
        b.feed(c, &mut out);
        heard += out
            .audio_runs()
            .map(|p| p.samples.len() / usize::from(p.channels.max(1)))
            .sum::<usize>();
        eb.append(&mut out.events);
        out.clear();
    }
    b.finish(&mut out);
    eb.append(&mut out.events);
    assert_eq!(frames(&ea), frames(&eb));
    assert!(
        (heard as f64 - 16.0 * 8000.0).abs() < 16.0 * 400.0,
        "{heard}"
    );
    let r = a.results();
    let id = r.decl().series_id(WATERFALL_SERIES).unwrap();
    assert!(!r.series(id).unwrap().points.is_empty());
}
