//! Marine VHF: generator -> receiver, the call log, the player and the scan.

use crate::common::*;
use sdrkit_core::publish::keys;
use sdrkit_core::{Generator, HostRequest, Value};
use sdrkit_presets::marine::MarinePlan;
use sdrkit_stack_marine::vhf::{self, VhfGen, VhfGenerator};
use sdrkit_stack_marine::vhf::{VhfCallEvent, VhfChannelRow, CALL};
use sdrkit_stack_marine::MARINEVHF;
use serde_json::json;
use std::sync::OnceLock;

/// The international plan.
fn int() -> &'static MarinePlan {
    static P: OnceLock<MarinePlan> = OnceLock::new();
    P.get_or_init(|| plan_of("INT"))
}

/// The ship frequency of the international channel `label`, Hz.
fn hz(label: &str) -> f64 {
    int()
        .by_label(label)
        .expect("a channel of the plan")
        .ship_hz
}

/// The setting `plan` of the international plan.
fn int_plan() -> (&'static str, Json) {
    ("plan", json!(int().plan_text()))
}

/// The setting `scan_hz` of the international channels `labels`.
fn scan(labels: &[&str]) -> (&'static str, Json) {
    let list: Vec<String> = labels.iter().map(|l| hz(l).to_string()).collect();
    ("scan_hz", json!(list.join(" ")))
}

/// The signal of `o` for `secs`, with noise at 30 dB SNR in 12.5 kHz from the engine's impairments.
fn render(o: VhfGen, secs: f64) -> Vec<sdrkit_core::Complex32> {
    let mut g = impaired(Box::new(VhfGenerator::new(o)), &snr(30.0));
    generate(g.as_mut(), secs)
}

/// The channel that plays now, from the typed results.
fn playing(r: &sdrkit_core::Results) -> Option<String> {
    text(r, "playing")
}

#[test]
fn the_generator_has_unit_power_while_it_sends() {
    let o = VhfGen {
        tx: vec![tx("16", 0.0, 1.0, 1000.0)],
        ..Default::default()
    };
    let mut g = VhfGenerator::new(o);
    let mut x = vec![sdrkit_core::Complex32::default(); 100_000];
    g.generate(&mut x);
    let p = x.iter().map(|v| v.norm_sqr() as f64).sum::<f64>() / x.len() as f64;
    assert!((p - 1.0).abs() < 1e-3, "{p}");
    assert_eq!(g.signal_info(), Some(vhf::signal_info()));
}

#[test]
fn three_transmissions_give_three_calls() {
    let o = VhfGen::default();
    let (rate, centre) = (o.rate, o.centre_hz);
    let x = render(o, 10.0);
    let mut rx = receiver(&MARINEVHF, None, &[int_plan(), ("priority_hz", json!(0.0))]);
    configure(rx.as_mut(), rate, centre, centre - 12.5e3);
    let ev = run(rx.as_mut(), &x, 65_536);
    check_events(
        &sdrkit_stack_marine::typed::shape("marinevhf").results_decl(),
        &ev,
    );
    let calls: Vec<_> = ev.iter().filter(|e| e.is(&CALL)).collect();
    assert_eq!(calls.len(), 3, "{}", rx.results().status().summary);
    let want = [
        ("16", 156.8e6, 1.0),
        ("6", 156.3e6, 3.5),
        ("13", 156.65e6, 6.0),
    ];
    for (c, (l, f, t)) in calls.iter().zip(want) {
        assert_eq!(c.get(&keys::CHANNEL), Some(l));
        assert!((c.freq_hz - f).abs() < 1.0);
        let start = c.at(&VhfCallEvent::START).unwrap();
        assert!(start - t > -0.4 && start - t < 0.6, "{start}");
        let d = c.at(&VhfCallEvent::DURATION).unwrap();
        assert!((d - 2.0).abs() < 0.7, "{d}");
        let pk = c.at(&VhfCallEvent::PEAK_DBFS).unwrap();
        assert!((-15.0..6.0).contains(&pk), "{pk}");
    }
    // The call table lists them with the same channels, and the channel table counts one call on each.
    assert_eq!(rows(rx.results(), "calls").len(), 3);
    assert_eq!(count(rx.results(), "calls"), 3);
    let ch = rows(rx.results(), "channels");
    for l in ["16", "6", "13"] {
        let row = ch
            .iter()
            .find(|r| r.get(&keys::CHANNEL) == Some(l))
            .expect("a channel row");
        assert_eq!(row.at(&VhfChannelRow::CALLS), Some(1), "{l}");
    }
    // The 1 MHz capture holds many channels.
    assert_eq!(flag(rx.results(), "multi_channel"), Some(true));
    // The view setting lists only the channels heard.
    apply(rx.as_mut(), "show_heard", Value::Bool(true));
    let heard = typed_rows(rx.results(), "channels");
    assert_eq!(heard.len(), 3, "{heard:?}");
}

#[test]
fn noise_alone_returns_no_call() {
    let o = VhfGen {
        tx: vec![],
        ..Default::default()
    };
    let (rate, centre) = (o.rate, o.centre_hz);
    // No transmission: the noise has the level it has under a unit-power signal at 30 dB.
    let mut g = impaired(
        Box::new(VhfGenerator::new(o)),
        &[
            ("gen_noise", json!("floor")),
            (
                "gen_noise_floor_dbfs",
                json!(-30.0 + 10.0 * (1e6f64 / 12_500.0).log10()),
            ),
        ],
    );
    let x = generate(g.as_mut(), 4.0);
    let mut rx = receiver(&MARINEVHF, None, &[]);
    configure(rx.as_mut(), rate, centre, centre);
    let ev = run(rx.as_mut(), &x, 65_536);
    assert!(ev.iter().all(|e| !e.is(&CALL)));
    assert_eq!(count(rx.results(), "calls"), 0);
}

#[test]
fn priority_16_takes_over() {
    let o = VhfGen {
        tx: vec![
            sdrkit_stack_marine::vhf::Transmission {
                freq_hz: hz("6"),
                start_s: 0.5,
                dur_s: 6.0,
                tone_hz: 700.0,
            },
            sdrkit_stack_marine::vhf::Transmission {
                freq_hz: hz("16"),
                start_s: 2.0,
                dur_s: 1.0,
                tone_hz: 1000.0,
            },
        ],
        ..Default::default()
    };
    let (rate, centre) = (o.rate, o.centre_hz);
    let x = render(o, 8.0);
    let mut rx = receiver(
        &MARINEVHF,
        None,
        &[int_plan(), ("channel_hz", json!(hz("6")))],
    );
    configure(rx.as_mut(), rate, centre, centre);
    let mut seen = Vec::new();
    for (k, c) in x.chunks((rate * 0.25) as usize).enumerate() {
        let mut out = sdrkit_core::Output::default();
        rx.feed(c, &mut out);
        seen.push((k as f64 * 0.25, playing(rx.results())));
    }
    let at = |t: f64| {
        seen.iter()
            .find(|s| (s.0 - t).abs() < 0.13)
            .and_then(|s| s.1.clone())
    };
    assert_eq!(at(1.5).as_deref(), Some("6"));
    assert_eq!(at(2.75).as_deref(), Some("16"));
    assert_eq!(at(6.0).as_deref(), Some("6"));
}

fn tx(
    label: &str,
    start_s: f64,
    dur_s: f64,
    tone_hz: f64,
) -> sdrkit_stack_marine::vhf::Transmission {
    sdrkit_stack_marine::vhf::Transmission {
        freq_hz: hz(label),
        start_s,
        dur_s,
        tone_hz,
    }
}

/// Runs a whole-span capture and returns (time, playing channel) after every 50 ms of signal.
fn playing_over_time(
    txs: Vec<sdrkit_stack_marine::vhf::Transmission>,
    secs: f64,
    set: &[(&str, Json)],
) -> Vec<(f64, Option<String>, String)> {
    let o = VhfGen {
        tx: txs,
        ..Default::default()
    };
    let (rate, centre) = (o.rate, o.centre_hz);
    let x = render(o, secs);
    let mut set = set.to_vec();
    set.push(int_plan());
    let mut rx = receiver(&MARINEVHF, None, &set);
    configure(rx.as_mut(), rate, centre, centre - 12.5e3);
    let step = (rate * 0.05) as usize;
    let mut seen = Vec::new();
    for (k, c) in x.chunks(step).enumerate() {
        let mut out = sdrkit_core::Output::default();
        rx.feed(c, &mut out);
        let r = rx.results();
        seen.push((
            (k + 1) as f64 * 0.05,
            playing(r),
            text(r, "scan_state").unwrap_or_default(),
        ));
    }
    seen
}

/// The report at `t` (the results are refreshed every 0.25 s: the first report at or after `t`).
fn report_at(seen: &[(f64, Option<String>, String)], t: f64) -> &(f64, Option<String>, String) {
    let tt = (t / 0.25 - 1e-9).ceil() * 0.25;
    seen.iter()
        .find(|s| (s.0 - tt).abs() < 0.01)
        .expect("a report")
}

fn at(seen: &[(f64, Option<String>, String)], t: f64) -> Option<String> {
    report_at(seen, t).1.clone()
}

/// Section 8.3 facts, whole span, scan off, priority on, nothing picked.
#[test]
fn whole_span_strongest_and_priority() {
    let s = playing_over_time(
        vec![tx("16", 1.0, 1.5, 1000.0), tx("13", 4.0, 1.5, 1300.0)],
        7.5,
        &[],
    );
    assert_eq!(at(&s, 0.5), None);
    assert_eq!(at(&s, 1.4).as_deref(), Some("16"));
    assert_eq!(at(&s, 2.0).as_deref(), Some("16"));
    assert_eq!(at(&s, 4.5).as_deref(), Some("13"));
    assert_eq!(at(&s, 7.0), None);
}

/// Section 8.3 facts: listening to 6 while it is quiet; 16 takes over, is held through its hang, then back on 6; priority off
/// stays on 6.
#[test]
fn picked_channel_and_priority_hang() {
    let txs = || vec![tx("16", 1.5, 2.0, 1000.0)];
    let s = playing_over_time(txs(), 7.5, &[("channel_hz", json!(hz("6")))]);
    assert_eq!(at(&s, 1.0).as_deref(), Some("6"));
    assert_eq!(at(&s, 2.0).as_deref(), Some("16"));
    assert_eq!(at(&s, 4.6).as_deref(), Some("16"));
    assert_eq!(at(&s, 6.9).as_deref(), Some("6"));
    let s = playing_over_time(
        txs(),
        7.5,
        &[("channel_hz", json!(hz("6"))), ("priority_hz", json!(0.0))],
    );
    for t in [1.0, 2.0, 4.6, 6.9] {
        assert_eq!(at(&s, t).as_deref(), Some("6"), "at {t}");
    }
}

/// Section 8.3 facts, whole-span scan: nothing open at 0.5 s; stops on 6; moves on to 13 after 6's hang.
#[test]
fn whole_span_scan_stops_and_moves_on() {
    let s = playing_over_time(
        vec![tx("6", 1.5, 0.9, 700.0), tx("13", 4.0, 2.0, 1300.0)],
        6.5,
        &[("scan", json!(true)), scan(&["6", "13", "16"])],
    );
    assert_eq!(at(&s, 0.5), None);
    assert_eq!(report_at(&s, 0.5).2, "scanning");
    assert_eq!(at(&s, 2.0).as_deref(), Some("6"));
    assert_eq!(report_at(&s, 2.0).2, "holding");
    assert_eq!(at(&s, 4.7).as_deref(), Some("13"));
    // Priority: 16 interrupts the scan and the interrupted channel resumes.
    let s = playing_over_time(
        vec![tx("6", 1.0, 5.0, 700.0), tx("16", 2.0, 1.0, 1000.0)],
        6.0,
        &[
            ("scan", json!(true)),
            scan(&["6", "13", "16"]),
            ("scan_hang_s", json!(0.5)),
        ],
    );
    assert_eq!(at(&s, 1.5).as_deref(), Some("6"));
    assert_eq!(at(&s, 2.5).as_deref(), Some("16"));
    assert_eq!(report_at(&s, 2.5).2, "priority");
    assert_eq!(at(&s, 5.0).as_deref(), Some("6"));
}

/// Single tuner: the capture holds one part of the list, so the scan steps the radio channel by channel (a retune request each
/// time), stops on the first active channel, holds it until it has been quiet for the hang time, and visits the
/// priority channel every 2 s for 0.3 s.
#[test]
fn single_tuner_scan_steps_the_radio() {
    let rate = 250e3;
    let txs = vec![tx("13", 1.5, 3.0, 1300.0)];
    let mut rx = receiver(
        &MARINEVHF,
        None,
        &[
            int_plan(),
            ("scan", json!(true)),
            scan(&["6", "13", "72"]),
            ("scan_dwell_s", json!(0.4)),
            ("scan_hang_s", json!(1.0)),
        ],
    );
    // The radio starts on channel 6 (156.300 MHz), the capture 12.5 kHz above it.
    let mut dial = 156.3e6;
    let opts = |centre: f64| VhfGen {
        rate,
        centre_hz: centre,
        tx: txs.clone(),
        ..Default::default()
    };
    rx.configure(&sdrkit_core::InputInfo::new(rate, dial + 12.5e3, dial))
        .unwrap();
    let noisy = |g: VhfGenerator| impaired(Box::new(g), &snr(30.0));
    let mut g = noisy(VhfGenerator::new(opts(dial + 12.5e3)));
    let plan = int();
    let step = (rate * 0.01) as usize;
    let mut buf = vec![sdrkit_core::Complex32::default(); step];
    let mut tunes = Vec::new();
    let mut visits: Vec<(f64, f64)> = Vec::new();
    let mut held_13 = Vec::new();
    for k in 0..800 {
        let t = (k + 1) as f64 * 0.01;
        g.generate(&mut buf);
        let mut out = sdrkit_core::Output::default();
        rx.feed(&buf, &mut out);
        for r in &out.requests {
            let HostRequest::Retune { tuned_hz: f, .. } = r else {
                panic!("{r:?}");
            };
            let label = plan
                .by_frequency(*f)
                .map(|(c, _)| c.label.clone())
                .unwrap_or_default();
            tunes.push((t, *f, label));
            if (f - dial).abs() > 1.0 {
                dial = *f;
                // The host moves the radio. The signal goes on from now.
                rx.retuned(&sdrkit_core::InputInfo::new(rate, dial + 12.5e3, dial));
                g = noisy(VhfGenerator::starting_at(opts(dial + 12.5e3), t));
                visits.push((t, dial));
            }
        }
        if (dial - 156.65e6).abs() < 1.0 && playing(rx.results()).as_deref() == Some("13") {
            held_13.push(t);
        }
    }
    // It stepped through the list before 13 came on.
    let before: Vec<&str> = tunes
        .iter()
        .filter(|x| x.0 < 1.5)
        .map(|x| x.2.as_str())
        .collect();
    assert!(
        before.contains(&"13") && before.contains(&"72"),
        "{before:?}"
    );
    // It visited the priority channel 16, for about 0.3 s each time.
    let probes: Vec<usize> = (0..visits.len())
        .filter(|&i| (visits[i].1 - 156.8e6).abs() < 1.0)
        .collect();
    assert!(!probes.is_empty());
    for &i in &probes {
        if let Some(n) = visits.get(i + 1) {
            let d = n.0 - visits[i].0;
            assert!((0.25..0.45).contains(&d), "probe of {d} s");
        }
    }
    // It stopped on 13 while 13 was active and played it (apart from the looks at 16), and came back to it after each look.
    assert!(held_13.len() > 150, "{} x 10 ms on 13", held_13.len());
    let during: Vec<&(f64, f64, String)> =
        tunes.iter().filter(|x| x.0 > 2.2 && x.0 < 4.4).collect();
    assert!(during.iter().any(|x| x.2 == "16"), "{during:?}");
    for w in during.windows(2) {
        if w[0].2 == "16" {
            assert_eq!(w[1].2, "13");
        }
    }
    assert!(held_13.iter().any(|t| *t > 4.0));
    // After 13 went quiet (4.5 s) and the hang, it went on scanning.
    assert!(tunes.iter().any(|x| x.0 > 5.0 && x.2 != "13"));
}

/// A clean test signal (no noise at all) produces the three calls and no others: the leakage of the window does not count as
/// carriers.
#[test]
fn a_clean_signal_returns_three_calls() {
    let o = VhfGen::default();
    let (rate, centre) = (o.rate, o.centre_hz);
    let mut g = VhfGenerator::new(o);
    let x = generate(&mut g, 9.0);
    let mut rx = receiver(&MARINEVHF, None, &[]);
    configure(rx.as_mut(), rate, centre, centre - 12.5e3);
    let _ = run(rx.as_mut(), &x, 65_536);
    assert_eq!(typed_rows(rx.results(), "calls").len(), 3);
}
