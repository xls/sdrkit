//! The graph components in a graph.

use crate::common::ook_iq;
use sdrkit_core::{Complex32, Output};
use sdrkit_graph::{Data, Format, GraphBuilder, Item, Registry, Timed};
use sdrkit_proto_ookdev::{
    BitRows, Check, Coding, Ctx, Modulation, ProtocolDef, ProtocolList, Reading, Timing, Train,
};
use std::sync::Arc;

fn remote(rows: &BitRows, _: &mut Ctx, out: &mut Vec<Reading>) {
    if rows.len(0) == 24 {
        out.push(Reading::new(
            "Remote",
            "remote",
            format!("{:06X}", rows.bits_u64(0, 0, 24)),
            Check::Repeat,
        ));
    }
}

static REMOTE: ProtocolDef = ProtocolDef {
    name: "Remote",
    modulation: Modulation::Ook,
    timing: Timing::new(Coding::Pwm, 464.0, 1404.0, 1800.0).with_tolerance(200.0),
    despike_us: 0.0,
    decode: remote,
};

fn items<T: Item>(d: &Data) -> Vec<Timed<T>> {
    match d {
        Data::Items(Some(c)) => c
            .as_any()
            .downcast_ref::<Vec<Timed<T>>>()
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[test]
fn registry_names() {
    let mut r = Registry::new();
    sdrkit_proto_ookdev::register(&mut r);
    sdrkit_proto_ookdev::register_decoder(&mut r, Arc::new(ProtocolList::new()));
    let names: Vec<&str> = r.names().collect();
    assert_eq!(
        names,
        vec!["ookdev.burst_pulses", "ookdev.decode", "ookdev.pulses"]
    );
    for n in names {
        assert_eq!(r.make(n).unwrap().info().name, n);
    }
}

#[test]
fn pulses_then_decode_in_a_graph() {
    let mut list = ProtocolList::new();
    list.add_static(&REMOTE);
    let mut reg = Registry::new();
    sdrkit_proto_ookdev::register(&mut reg);
    sdrkit_proto_ookdev::register_decoder(&mut reg, Arc::new(list));
    let mut g = GraphBuilder::new()
        .add_boxed("pulses", reg.make("ookdev.pulses").unwrap())
        .add_boxed("decode", reg.make("ookdev.decode").unwrap())
        .input("pulses")
        .connect("pulses", "decode")
        .output("decode.readings")
        .output("decode.unknown")
        .build()
        .unwrap();
    // A 24-bit PWM code: a 1 is a short mark (464 µs then 1404 µs), a 0 a long one. A pause follows the frame.
    let code = 0xC0FFEEu32;
    let pairs: Vec<(f32, f32)> = (0..24)
        .map(|k| {
            if (code >> (23 - k)) & 1 == 1 {
                (464.0, 1404.0)
            } else {
                (1404.0, 464.0)
            }
        })
        .collect();
    let mut x = ook_iq(&pairs, 1e6, 0.3, 2000.0, 1e-6, 5);
    x.extend(std::iter::repeat_n(Complex32::default(), 20_000));
    g.configure_input(Format::iq(1e6, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    let mut readings = Vec::new();
    for p in x.chunks(4096) {
        out.clear();
        g.process(p, &mut out);
        readings.extend(items::<Reading>(&g.buffer("decode.readings").unwrap().data));
    }
    assert_eq!(readings.len(), 1);
    assert_eq!(readings[0].value.id, "C0FFEE");
    assert!((readings[0].time - 2e-3).abs() < 1e-5);
    let _: Vec<Timed<Train>> = items(&g.buffer("decode.unknown").unwrap().data);
}
