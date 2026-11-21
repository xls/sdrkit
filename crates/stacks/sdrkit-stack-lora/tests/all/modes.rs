//! The descriptors, the rate refusal, retune and reset, and the component registry.

use crate::common::{count, invoke, rows};
use sdrkit_core::{Complex32, GenConfig, InputInfo, Output, StackConfig, Values};
use sdrkit_stack_lora::typed::{MeshPacketRow, MESH_NODES, MESH_PACKETS};

#[test]
fn every_stack_has_a_descriptor_settings_and_a_generator() {
    let ds = sdrkit_stack_lora::descriptors();
    let ids: Vec<&str> = ds.iter().map(|d| d.id).collect();
    assert_eq!(
        ids,
        [
            "mesh",
            "meshtastic",
            "meshcore",
            "lorawan",
            "loraaprs",
            "meshcom",
            "lorawanwide",
            "loraraw",
            "aprs",
            "fanet",
            "reticulum",
            "elrs"
        ]
    );
    for d in ds {
        let s = sdrkit_dev_test::Harness::new(d).values().unwrap();
        assert!(!(d.settings)().is_empty());
        let mut rx = (d.make_receiver)(&StackConfig::new(s.clone())).unwrap();
        // A rate below the minimum is refused with the stack's name.
        assert!(rx
            .configure(&InputInfo::new(
                100e3,
                d.tuning.default_hz,
                d.tuning.default_hz
            ))
            .is_err());
        let why = &rx.results().status().summary;
        assert!(
            why.contains("needs a sample rate of at least"),
            "{}: {why}",
            d.id
        );
        let mut g = (d.make_generator.unwrap())(&GenConfig::new(d.tuning.rate_hz, s).with_seed(1))
        .unwrap();
        let mut x = vec![Complex32::default(); 1000];
        assert_eq!(g.generate(&mut x), 1000);
    }
    let mut r = sdrkit_graph::Registry::default();
    sdrkit_stack_lora::register(&mut r);
    for name in [
        "lora.bank",
        "mesh.channels",
        "mesh.decode",
        "mt.client",
        "aprs.tables",
    ] {
        assert!(r.names().any(|n| n == name), "{name}");
    }
    assert_eq!(sdrkit_stack_lora::descriptors().len(), 12);
}

#[test]
fn reset_keeps_the_tables_and_clear_empties_them() {
    let d = &sdrkit_stack_lora::MESHTASTIC;
    let rate = 2e6;
    let s: Values = sdrkit_dev_test::Harness::new(d).values().unwrap();
    let mut g = (d.make_generator.unwrap())(&GenConfig::new(rate, s.clone()).with_submode_opt(Some("meshtastic".into())).with_seed(1))
    .unwrap();
    let mut x = vec![Complex32::default(); (rate * 6.0) as usize];
    g.generate(&mut x);
    let mut rx = (d.make_receiver)(&StackConfig::new(s).with_submode("meshtastic")).unwrap();
    let inp = InputInfo::new(rate, 869.825e6, 869.525e6).recording();
    rx.configure(&inp).unwrap();
    let mut out = Output::default();
    for c in x.chunks(50_000) {
        rx.feed(c, &mut out);
    }
    let nodes = rows(rx.results(), &MESH_NODES).len();
    let first_time = rows(rx.results(), &MESH_PACKETS)
        .last()
        .unwrap()
        .at(&MeshPacketRow::TIME_S);
    assert!(nodes > 0);
    rx.retuned(&inp);
    for c in vec![Complex32::default(); 500_000].chunks(50_000) {
        rx.feed(c, &mut out);
    }
    let res = rx.results();
    assert_eq!(rows(res, &MESH_NODES).len(), nodes);
    assert_eq!(
        res.fields().get(&sdrkit_core::publish::keys::FRAMES_OK),
        Some(0)
    );
    assert_eq!(
        rows(res, &MESH_PACKETS)
            .last()
            .unwrap()
            .at(&MeshPacketRow::TIME_S),
        first_time
    );
    invoke(rx.as_mut(), "clear", &[]).unwrap();
    for c in vec![Complex32::default(); 300_000].chunks(50_000) {
        rx.feed(c, &mut out);
    }
    let res = rx.results();
    assert_eq!(count(res, &MESH_NODES, |_| true), 0);
    assert_eq!(count(res, &MESH_PACKETS, |_| true), 0);
}
