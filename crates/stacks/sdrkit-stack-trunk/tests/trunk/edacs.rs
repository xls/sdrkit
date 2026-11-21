//! EDACS: the recorded vector, a generated site with calls, and following a call through the host.

use crate::common::*;
use num_complex::Complex32;
use sdrkit_core::{HostRequest, Receiver, Results, Value};
use sdrkit_stack_trunk::edacs::gen::{d14_calls, EdacsSite};
use sdrkit_stack_trunk::typed::{EDACS_CALLS, EDACS_TALKGROUPS};
use sdrkit_stack_trunk::{edacs, EDACS};

fn site_rx(site: &EdacsSite, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    let table = site.table_text();
    let mut s: Vec<(&str, &str)> = vec![("lcn_table", table.as_str())];
    s.extend_from_slice(settings);
    let mut rx = make(&EDACS, &s);
    rx.configure(&sdrkit_core::InputInfo::new(site.rate, site.centre_hz, site.cc_hz).recording())
        .unwrap();
    rx
}

/// (LCN, group or called unit, calling unit, emergency, carrier only) of every call.
fn calls(r: &Results) -> Vec<(String, String, String, String, String)> {
    rows(r, &EDACS_CALLS)
        .into_iter()
        .map(|c| {
            let get = |k: &str| c.get(k).cloned().unwrap_or_default();
            let emergency = if get("urgency") == "emergency" {
                "yes"
            } else {
                "no"
            };
            (
                get("channel"),
                get("target"),
                get("source"),
                emergency.to_string(),
                get("carrier_only"),
            )
        })
        .collect()
}

#[test]
fn recorded_vector_matches_expected() {
    let Some(rec) = sdrkit_dev_test::read_recording("trunk/edacs_system_307k") else {
        return;
    };
    let exp = sdrkit_dev_test::sigmf::read_expected("trunk").unwrap();
    let e = &exp["recordings"]["edacs_system_307k"];
    let table: String = e["settings"]["lcn_table_mhz"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| format!("{}\n", f.as_f64().unwrap()))
        .collect();
    let mut rx = make(
        &EDACS,
        &[
            ("lcn_table", &table),
            ("follow", "true"),
            ("follow_carriers", "true"),
        ],
    );
    rx.configure(
        &sdrkit_core::InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz - 100e3).recording(),
    )
    .unwrap();
    let fed = feed(rx.as_mut(), &rec.samples, 4096);
    let r = results(rx.as_ref());
    let f = &e["final"];
    assert!(num(r, "frames_ok") >= 240.0, "{}", num(r, "frames_ok"));
    assert!(num(r, "frames_bad") <= 3.0);
    assert_eq!(num(r, "site"), f["site"].as_f64().unwrap());
    assert_eq!(num(r, "control_lcn"), f["control_lcn"].as_f64().unwrap());
    assert_eq!(num(r, "highest_lcn"), f["highest_lcn"].as_f64().unwrap());
    assert_eq!(text(r, "polarity"), f["polarity"].as_str().unwrap());
    let c = calls(r);
    for w in e["calls"].as_array().unwrap() {
        let lcn = w["lcn"].to_string();
        let row = c.iter().find(|x| x.0 == lcn).expect("call row");
        if w["gid"].as_i64().unwrap() >= 0 {
            assert_eq!(row.1, w["gid"].to_string());
            assert_eq!(row.2, w["lid"].to_string());
        }
    }
    let tg = rows(r, &EDACS_TALKGROUPS);
    for w in e["talkgroups"].as_array().unwrap() {
        assert!(
            tg.iter().any(|t| t["gid"] == w["gid"].to_string().as_str()),
            "{tg:?}"
        );
    }
    // Every followed call is a host request with its frequency, and each follow event says when and where.
    let follows: Vec<f64> = fed
        .requests
        .iter()
        .filter_map(|r| match r {
            HostRequest::Follow { freq_hz, .. } => Some(*freq_hz),
            _ => None,
        })
        .collect();
    for lcn_hz in [856.05e6, 856.125e6, 856.175e6] {
        assert!(
            follows.iter().any(|f| (f - lcn_hz).abs() < 1.0),
            "{follows:?}"
        );
    }
    check_events(&kinds(&edacs::declaration()), &fed.events, true);
}

#[test]
fn the_site_calls_and_talkgroups() {
    let site = EdacsSite {
        calls: d14_calls(),
        ..Default::default()
    };
    let mut rx = site_rx(&site, &[]);
    let fed = feed(rx.as_mut(), &site.iq(), 8192);
    let c = calls(results(rx.as_ref()));
    assert!(
        c.iter()
            .any(|x| x.0 == "3" && x.1 == "777" && x.2 == "4321"),
        "{c:?}"
    );
    assert!(
        c.iter().any(|x| x.0 == "6" && x.1 == "888" && x.3 == "yes"),
        "{c:?}"
    );
    assert!(c.iter().any(|x| x.0 == "8" && x.4 == "yes"), "{c:?}");
    assert!(
        fed.requests.is_empty(),
        "nothing is followed when follow is off"
    );
    check_events(&kinds(&edacs::declaration()), &fed.events, true);
}

#[test]
fn edacs_follow_event_has_time_and_frequency() {
    let site = EdacsSite {
        calls: d14_calls(),
        ..Default::default()
    };
    let mut rx = site_rx(&site, &[("follow", "true")]);
    let mut x = site.iq();
    add_noise(&mut x, 0.01, 3);
    let fed = feed(rx.as_mut(), &x, 8192);
    let ev: Vec<_> = fed
        .events
        .iter()
        .filter(|e| e.kind() == "trunk.follow")
        .collect();
    assert!(!ev.is_empty(), "no follow event");
    for e in &ev {
        assert!(e.time > 0.5, "{e:?}");
        assert!(e.freq_hz > 856e6, "{e:?}");
    }
    let first = ev.iter().find(|e| field(e, "action") == "follow").unwrap();
    assert!((first.freq_hz - 856.05e6).abs() < 1.0, "{first:?}");
    assert!(fed
        .requests
        .iter()
        .any(|r| matches!(r, HostRequest::Release { .. })));
    for r in &fed.requests {
        r.validate().unwrap();
    }
}

#[test]
fn follow_groups_take_only_their_groups_and_refuse_a_bad_item() {
    let site = EdacsSite {
        calls: d14_calls(),
        ..Default::default()
    };
    let mut rx = site_rx(&site, &[("follow", "true"), ("follow_groups", "888")]);
    let fed = feed(rx.as_mut(), &site.iq(), 8192);
    let follows: Vec<f64> = fed
        .requests
        .iter()
        .filter_map(|r| match r {
            HostRequest::Follow { freq_hz, .. } => Some(*freq_hz),
            _ => None,
        })
        .collect();
    assert_eq!(follows, [856.125e6]);
    assert!(set(
        rx.as_mut(),
        "follow_groups",
        &Value::Text("777 eight".into())
    )
    .is_err());
}

#[test]
fn noise_alone_returns_nothing() {
    let mut x = vec![Complex32::new(0.0, 0.0); 480_000 * 3];
    add_noise(&mut x, 0.1, 7);
    let site = EdacsSite::default();
    let mut rx = site_rx(&site, &[]);
    feed(rx.as_mut(), &x, 8192);
    assert_eq!(num(results(rx.as_ref()), "frames_ok"), 0.0);
    assert!(calls(results(rx.as_ref())).is_empty());
}

/// The share of 100 unit assignments read, after 40 filler frames (1.2 s, the follow-up's settling time), at `snr_db` in
/// 12.5 kHz with the control channel `offset_hz` from where the receiver is tuned and the bit clock `clock` times its rate.
fn unit_share(snr_db: f64, offset_hz: f64, clock: f64, seed: u64) -> f64 {
    use sdrkit_stack_trunk::edacs::msg::build;
    let units = 100;
    let mut frames: Vec<(u32, u32)> = (0..40).map(|k| build::filler_std(0x1234 * k)).collect();
    frames.extend(
        (0..units).map(|k| build::group(2 + k % 8, 100 + k, 1000 + k, false, false, false)),
    );
    let site = EdacsSite {
        rate: 307.2e3,
        centre_hz: 856.0e6,
        cc_hz: 856.0e6 + offset_hz,
        seconds: 4.8,
        clock,
        frames: Some(frames),
        ..Default::default()
    };
    let mut x = site.iq();
    add_noise_snr(&mut x, site.rate, snr_db, 12.5e3, seed);
    let mut rx = make(&EDACS, &[("lcn_table", site.table_text().as_str())]);
    rx.configure(&sdrkit_core::InputInfo::new(site.rate, 856.0e6, 856.0e6).recording())
        .unwrap();
    let split = ((400.0 + 40.5 * 288.0) / 9600.0 * site.rate) as usize;
    feed(rx.as_mut(), &x[..split], 8192);
    let first = num(results(rx.as_ref()), "frames_ok");
    feed(rx.as_mut(), &x[split..], 8192);
    (num(results(rx.as_ref()), "frames_ok") - first) / units as f64
}

#[test]
fn ninety_percent_of_units_at_the_required_levels() {
    // At least 90 % of the units at 11.3 dB (AWGN), 11.5 dB (+3.1 kHz), 12.3 dB (-6 kHz), 11.3 dB (50 ppm), 5 noise seeds.
    let mut lines = Vec::new();
    for (what, offset, clock, snr) in [
        ("awgn", 0.0, 1.0, 11.3),
        ("+3.1 kHz", 3100.0, 1.0, 11.5),
        ("-6 kHz", -6000.0, 1.0, 12.3),
        ("50 ppm", 0.0, 1.0 + 50e-6, 11.3),
    ] {
        let share = (1..=5)
            .map(|s| unit_share(snr, offset, clock, s))
            .sum::<f64>()
            / 5.0;
        lines.push(format!("{what} {snr} dB: {share:.3}"));
        assert!(share >= 0.9, "{lines:?}");
    }
    eprintln!("{}", lines.join("\n"));
}
