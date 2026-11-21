use sdrkit_components::testing::{run_passes, Source};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Format, GraphBuilder, Kind, Registry};
use sdrkit_proto_link::components::register;
use sdrkit_proto_link::modem::{afsk1200_audio, g3ruh9600_audio};

fn frame() -> Vec<u8> {
    let f = sdrkit_proto_link::ax25::Frame::from_tnc2(
        "N0CALL-9>APRS,WIDE1-1:!4903.50N/07201.75W-graph test",
    )
    .unwrap();
    let mut b = Vec::new();
    f.encode(&mut b).unwrap();
    b
}

fn run(name: &str, audio: Vec<f32>) -> Vec<Vec<u8>> {
    let mut r = Registry::new();
    register(&mut r);
    let comp = r.make(name).unwrap();
    assert_eq!(comp.info().name, name);
    let mut g = GraphBuilder::new()
        .add(
            "src",
            Source::new(Format::new(Kind::Real, 48_000.0), Data::Real(audio), 4096),
        )
        .add_boxed("demod", comp)
        .connect("src", "demod")
        .build()
        .unwrap();
    match run_passes(&mut g, 64, "demod.out") {
        Data::Bytes(p) => p.iter().map(|p| p.bytes().to_vec()).collect(),
        d => panic!("{d:?}"),
    }
}

#[test]
fn link_components_decode_in_a_graph() {
    let f = frame();
    let mut audio = vec![0.0; 1000];
    afsk1200_audio(&[&f], 20, 48_000.0, &mut audio).unwrap();
    assert_eq!(run("link.afsk1200", audio), std::slice::from_ref(&f));
    let mut audio = vec![0.0; 1000];
    g3ruh9600_audio(&[&f], 20, 48_000.0, &mut audio).unwrap();
    assert_eq!(run("link.g3ruh9600", audio), [f]);
}
