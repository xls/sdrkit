//! Optional inputs: a graph may leave them unconnected, and the component then uses its documented default.

use sdrkit_core::{Complex32, ConfigError, InputInfo, Receiver};
use sdrkit_graph::{
    Buffer, Component, ComponentInfo, Data, Format, GraphBuilder, GraphError, Io, Kind, Port,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// `Iq` in, `Real` out: the magnitude times a gain. The gain is the last value read on the optional `gain` input, or 2 when
/// that input is unconnected. Counts the calls that saw the input unconnected.
struct Scale {
    gain: f32,
    unconnected_calls: Arc<AtomicU64>,
}

impl Component for Scale {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "test.scale",
            vec![Port::iq("in"), Port::real("gain").optional("a gain of 2")],
            vec![Port::real("out")],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        assert_eq!(inputs[1].kind, Kind::Real);
        Ok(vec![Format::new(Kind::Real, inputs[0].rate)])
    }
    fn process(&mut self, io: &mut Io) {
        if io.is_connected(1) {
            if let Some(&g) = io.real(1).last() {
                self.gain = g;
            }
        } else {
            assert!(io.real(1).is_empty(), "an unconnected input holds nothing");
            self.unconnected_calls.fetch_add(1, Ordering::Relaxed);
        }
        let x = io.iq(0);
        let g = self.gain;
        io.real_out(0).extend(x.iter().map(|s| s.norm() * g));
    }
}

/// `Iq` in, `Real` out: a constant 3 per sample.
struct Three;
impl Component for Three {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.three", vec![Port::iq("in")], vec![Port::real("out")])
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Real, inputs[0].rate)])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.input_len(0);
        io.real_out(0).extend(std::iter::repeat_n(3.0, n));
    }
}

fn scale() -> (Scale, Arc<AtomicU64>) {
    let n = Arc::new(AtomicU64::new(0));
    (
        Scale {
            gain: 2.0,
            unconnected_calls: n.clone(),
        },
        n,
    )
}

fn run(mut g: sdrkit_graph::Graph) -> Vec<f32> {
    let info = InputInfo {
        rate_hz: 1000.0,
        centre_hz: 0.0,
        tuned_hz: 0.0,
        live: false,
    };
    g.configure(&info).unwrap();
    let mut tap = g.tap("scale.out", 64).unwrap();
    let mut out = sdrkit_core::Output::default();
    g.feed(&[Complex32::new(1.0, 0.0); 10], &mut out);
    let (mut b, mut got) = (Buffer::default(), Vec::new());
    while tap.pop(&mut b) {
        if let Data::Real(x) = &b.data {
            got.extend_from_slice(x);
        }
    }
    got
}

#[test]
fn an_unconnected_optional_input_returns_the_default() {
    let (s, calls) = scale();
    let g = GraphBuilder::new()
        .add("scale", s)
        .input("scale")
        .build()
        .unwrap();
    assert_eq!(run(g), vec![2.0; 10]);
    assert!(calls.load(Ordering::Relaxed) > 0);
}

#[test]
fn a_connected_optional_input_is_read() {
    let (s, calls) = scale();
    let g = GraphBuilder::new()
        .add("three", Three)
        .add("scale", s)
        .input("three")
        .input("scale.in")
        .connect("three", "scale.gain")
        .build()
        .unwrap();
    assert_eq!(run(g), vec![3.0; 10]);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn a_required_input_must_still_be_connected_and_an_optional_one_must_have_a_kind() {
    struct AnyOptional;
    impl Component for AnyOptional {
        fn values(&self) -> sdrkit_core::Values {
            sdrkit_graph::no_values()
        }
        fn info(&self) -> ComponentInfo {
            ComponentInfo::processor(
                "test.any_optional",
                vec![Port::iq("in"), Port::any("side").optional("nothing")],
                vec![Port::real("out")],
            )
        }
        fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
            Ok(vec![Format::new(Kind::Real, 0.0)])
        }
        fn process(&mut self, _: &mut Io) {}
    }
    let (s, _) = scale();
    let e = GraphBuilder::new().add("scale", s).build().unwrap_err();
    assert_eq!(e, GraphError::Unconnected("scale.in".into()));
    let e = GraphBuilder::new()
        .add("x", AnyOptional)
        .input("x")
        .build()
        .unwrap_err();
    assert!(e.to_string().contains("must declare its kind"), "{e}");
}

#[test]
fn a_standalone_io_can_mark_an_input_unconnected() {
    let (mut s, calls) = scale();
    let inputs = [Buffer::new(&Kind::Iq), Buffer::new(&Kind::Real)];
    let formats = [
        Format::new(Kind::Iq, 1000.0),
        Format::new(Kind::Real, 0.0),
        Format::new(Kind::Real, 1000.0),
    ];
    let mut outs = [Buffer::new(&Kind::Real)];
    let mut io = Io::standalone(&inputs, &formats, &[0, 1], &[2], &mut outs).with_unconnected(1);
    assert!(io.is_connected(0) && !io.is_connected(1));
    s.process(&mut io);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
