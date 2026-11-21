//! The VHF stacks with the channel plans of two rows of the data file: the labels follow the plan that the host gives.

use crate::common::*;
use sdrkit_core::publish::keys;
use sdrkit_stack_marine::atis::{AtisGen, AtisGenerator};
use sdrkit_stack_marine::vhf::{Transmission, VhfGen, VhfGenerator, CALL};
use sdrkit_stack_marine::{ATIS, MARINEVHF};
use serde_json::json;

/// Channel 22 of the international plan is the US channel 22A (157.100 MHz ship side).
const CH22_HZ: f64 = 157.1e6;

#[test]
fn marine_vhf_labels_a_call_by_the_plan_given() {
    let o = VhfGen {
        centre_hz: CH22_HZ - 0.2e6,
        tx: vec![Transmission {
            freq_hz: CH22_HZ,
            start_s: 0.5,
            dur_s: 2.0,
            tone_hz: 1000.0,
        }],
        ..Default::default()
    };
    let (rate, centre) = (o.rate, o.centre_hz);
    let mut g = impaired(Box::new(VhfGenerator::new(o)), &snr(30.0));
    let x = generate(g.as_mut(), 4.0);
    for (id, label) in [("INT", "22"), ("USA", "22A")] {
        let plan = plan_of(id);
        assert_eq!(
            plan.by_frequency(CH22_HZ).map(|(c, _)| c.label.as_str()),
            Some(label)
        );
        let mut rx = receiver(&MARINEVHF, None, &[("plan", json!(plan.plan_text()))]);
        configure(rx.as_mut(), rate, centre, centre - 12.5e3);
        let ev = run(rx.as_mut(), &x, 65_536);
        let calls: Vec<_> = ev.iter().filter(|e| e.is(&CALL)).collect();
        assert_eq!(calls.len(), 1, "{id}");
        assert_eq!(calls[0].get(&keys::CHANNEL), Some(label), "{id}");
    }
}

#[test]
fn atis_names_the_channel_of_the_plan_given() {
    let rate = 1e6;
    let o = AtisGen {
        rate,
        channel_hz: -150e3,
        ..Default::default()
    };
    let mut g = impaired(Box::new(AtisGenerator::new(o)), &snr(30.0));
    let x = generate(g.as_mut(), 8.0);
    for (id, label) in [("INT", "22"), ("USA", "22A")] {
        let mut rx = receiver(&ATIS, None, &[("plan", json!(plan_of(id).plan_text()))]);
        configure(rx.as_mut(), rate, CH22_HZ + 150e3, CH22_HZ);
        let ev = run(rx.as_mut(), &x, 100_000);
        let msgs: Vec<_> = ev.iter().filter(|e| e.kind() == "atis.message").collect();
        assert!(!msgs.is_empty(), "{id}");
        assert!(
            msgs.iter().all(|e| e.get(&keys::CHANNEL) == Some(label)),
            "{id}"
        );
    }
}

/// The repository's data file at `rel`.
fn data_file(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(rel)
}

#[test]
fn every_plan_and_every_broadcast_of_the_files_is_taken() {
    let plans =
        sdrkit_presets::marine::read_plans(&data_file(sdrkit_presets::marine::VHF_CHANNELS))
            .unwrap();
    assert_eq!(plans.len(), 5);
    for p in &plans {
        for d in [&MARINEVHF, &ATIS] {
            let _ = receiver(d, None, &[("plan", json!(p.plan_text()))]);
        }
    }
    let broadcasts = sdrkit_presets::marine::read_wefax_broadcasts(&data_file(
        sdrkit_presets::marine::WEFAX_BROADCASTS,
    ))
    .unwrap();
    for b in &broadcasts {
        let set: Vec<(String, Json)> = b
            .settings()
            .into_iter()
            .map(|(k, v)| match v {
                sdrkit_core::Value::Text(t) => (k, json!(t)),
                v => panic!("{v:?}"),
            })
            .collect();
        let set: Vec<(&str, Json)> = set.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        let _ = receiver(&sdrkit_stack_marine::MARINE, Some("wefax"), &set);
    }
}

#[test]
fn a_scan_without_a_plan_runs() {
    let o = VhfGen::default();
    let (rate, centre) = (o.rate, o.centre_hz);
    let x = generate(&mut VhfGenerator::new(o), 1.0);
    let mut rx = receiver(&MARINEVHF, None, &[("scan", json!(true))]);
    configure(rx.as_mut(), rate, centre, centre - 12.5e3);
    let _ = run(rx.as_mut(), &x, 65_536);
}
