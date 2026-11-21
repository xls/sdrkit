//! Connections by handle and port index: refusals at connect, format rules at configure, descriptors and the stack build step.

use sdrkit_core::publish::field;
use sdrkit_core::{
    Complex32, ConfigError, InputInfo, Output, Receiver, Setting, SettingSet, Values,
};
use sdrkit_graph::components::*;
use sdrkit_graph::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn info(rate: f64) -> InputInfo {
    InputInfo {
        rate_hz: rate,
        centre_hz: 100e6,
        tuned_hz: 100e6,
        live: false,
    }
}

/// A sink with one input port, given at construction.
struct Takes(Port);

impl Component for Takes {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.takes", vec![self.0.clone()])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

/// A plan of channel, AM demodulator and audio resampler, with the handles.
fn audio_plan(r: &Registry) -> (GraphPlan, [ComponentHandle; 3]) {
    let mut p = GraphPlan::new();
    let chan = p.add(r, "chan", "dsp.channel").unwrap();
    let demod = p.add(r, "demod", "dsp.am_demod").unwrap();
    let audio = p.add(r, "audio", "dsp.audio_resampler").unwrap();
    p.input(PortRef::new(chan, 0)).unwrap();
    p.connect(PortRef::new(chan, 0), PortRef::new(demod, 0))
        .unwrap();
    p.connect(PortRef::new(demod, 0), PortRef::new(audio, 0))
        .unwrap();
    (p, [chan, demod, audio])
}

#[test]
fn audio_into_a_video_decoder_fails_at_connect() {
    let r = Registry::standard();
    let (mut p, [_, _, audio]) = audio_plan(&r);
    let h264 = [MediaSpec::any(Codec::H264)];
    let video = p
        .add_component("video", Box::new(Takes(Port::media("in", &h264))))
        .unwrap();
    let (from, to) = (PortRef::new(audio, 0), PortRef::new(video, 0));
    match p.connect(from, to) {
        Err(ConnectError::NoConversion {
            from: f,
            to: t,
            produces,
            accepts,
        }) => {
            assert_eq!((f, t), (from, to));
            assert_eq!(produces, Kind::Audio);
            assert_eq!(accepts.kind, KindRule::Is(Kind::Bytes));
            assert_eq!(accepts.media, h264);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_taken_or_missing_port_and_a_missing_component_fail_at_connect() {
    let r = Registry::standard();
    let (mut p, [chan, demod, audio]) = audio_plan(&r);
    let to = PortRef::new(demod, 0);
    assert_eq!(
        p.connect(PortRef::new(chan, 0), to),
        Err(ConnectError::PortTaken { to })
    );
    assert_eq!(
        p.connect(PortRef::new(chan, 0), PortRef::new(audio, 4)),
        Err(ConnectError::NoSuchPort {
            component: audio,
            side: Side::Input,
            port: 4,
        })
    );
    assert_eq!(
        p.connect(PortRef::new(chan, 3), PortRef::new(audio, 0)),
        Err(ConnectError::NoSuchPort {
            component: chan,
            side: Side::Output,
            port: 3,
        })
    );
    let ghost = ComponentHandle(99);
    assert_eq!(
        p.connect(PortRef::new(chan, 0), PortRef::new(ghost, 0)),
        Err(ConnectError::NoSuchComponent { component: ghost })
    );
}

/// The graph of the audio chain into `port`, built.
fn chain_into(port: Port) -> sdrkit_graph::Graph {
    GraphBuilder::new()
        .add("chan", Channel::new(10e3, 24e3).unwrap())
        .add("demod", AmDemod::default())
        .add("audio", AudioResampler::new(48e3).unwrap())
        .add("sink", Takes(port))
        .input("chan")
        .connect("chan", "demod")
        .connect("demod", "audio")
        .connect("audio", "sink")
        .build()
        .unwrap()
}

/// The kinds of the adapters that validate inserts behind the audio chain into `port`, at an input of 240 kHz.
fn adapters_for(port: Port) -> Vec<&'static str> {
    let mut g = chain_into(port);
    let r = g.validate(Format::new(Kind::Iq, 240e3)).unwrap();
    r.adapters.iter().map(|a| a.kind).collect()
}

#[test]
fn rate_and_channel_rules_insert_adapters_or_fail_at_validate() {
    // The demodulator returns real samples: the audio resampler reads them as mono audio.
    assert_eq!(adapters_for(Port::audio("in")), ["dsp.real_to_audio"]);
    assert_eq!(
        adapters_for(Port::audio("in").rate(RateRule::Exact(44.1e3))),
        ["dsp.real_to_audio", "dsp.resampler"]
    );
    assert_eq!(
        adapters_for(Port::audio("in").rate(RateRule::AtLeast(96e3))),
        ["dsp.real_to_audio", "dsp.resampler"]
    );
    assert_eq!(
        adapters_for(Port::audio("in").channels(ChannelRule::Exact(2))),
        ["dsp.real_to_audio", "dsp.duplicate"]
    );
    let mut g = chain_into(Port::audio("in").sample_type(SampleType::I16));
    match g.validate(Format::new(Kind::Iq, 240e3)) {
        Err(GraphError::Connect { error, .. }) => assert_eq!(
            *error,
            ConnectError::SampleTypeMismatch {
                from: PortRef::new(ComponentHandle(2), 0),
                to: PortRef::new(ComponentHandle(3), 0),
                sample_type: Some(SampleType::F32),
                accepts: SampleType::I16,
            }
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_registry_describes_every_kind_without_making_one() {
    static MADE: AtomicUsize = AtomicUsize::new(0);
    let mut r = Registry::standard();
    r.add("test.takes", || {
        MADE.fetch_add(1, Ordering::Relaxed);
        Box::new(Takes(Port::real("in")))
    });
    let made = MADE.load(Ordering::Relaxed);
    let names: Vec<String> = r.names().map(str::to_owned).collect();
    let kinds: Vec<&str> = r.descriptors().map(|d| d.kind.as_str()).collect();
    assert_eq!(names, kinds);
    let chan = r.descriptor("dsp.channel").unwrap();
    assert_eq!(chan.role, Role::Processor);
    assert_eq!(chan.inputs[0].format.kind, KindRule::Is(Kind::Iq));
    assert_eq!(chan.outputs[0].name, "out");
    assert!(chan.settings.id("rate").is_ok());
    let takes = r.descriptor("test.takes").unwrap();
    assert_eq!(takes.role, Role::Sink);
    assert_eq!(takes.inputs[0].format.kind, KindRule::Is(Kind::Real));
    assert_eq!(MADE.load(Ordering::Relaxed), made);
}

#[test]
fn a_component_is_described_with_its_constructor_values() {
    let build = |chan: Channel| {
        GraphBuilder::new()
            .add("chan", chan)
            .add("demod", AmDemod::default())
            .add("audio", AudioResampler::new(48e3).unwrap())
            .input("chan")
            .connect("chan", "demod")
            .connect("demod", "audio")
            .output("audio")
            .build()
            .unwrap()
    };
    let mut a = build(Channel::new(12e3, 24e3).unwrap().with_offset(30e3).unwrap());
    let d = a.describe();
    let s = &d.components[0].settings;
    assert_eq!(s["normal"], 12e3);
    assert_eq!(s["offset"], 30e3);
    assert_eq!(s["rate"], 24e3);
    let mut b = Graph::from_json(&d.to_json(), &Registry::standard()).unwrap();
    a.configure(&info(240e3)).unwrap();
    b.configure(&info(240e3)).unwrap();
    let x: Vec<Complex32> = (0..24_000)
        .map(|i| Complex32::from_polar(0.3, i as f32 * 0.8))
        .collect();
    let (mut oa, mut ob) = (Output::default(), Output::default());
    a.feed(&x, &mut oa);
    b.feed(&x, &mut ob);
    assert!(!oa.audio().is_empty());
    assert_eq!(oa.audio(), ob.audio());
}

const NOWHERE: Setting<field::F64> = Setting::new("nowhere", "Nowhere", "", 1.0);
sdrkit_core::settings! {
    /// A stack setting that no component takes.
    enum LostSetting {
        Nowhere = NOWHERE,
    }
}

#[test]
fn a_stack_setting_that_resolves_to_no_component_fails_at_build() {
    let g = GraphBuilder::new()
        .add("chan", Channel::default())
        .input("chan")
        .add("sink", NullSink::default())
        .connect("chan", "sink")
        .build()
        .unwrap();
    let e = StackGraph::builder("lost", g)
        .with_schema(LostSetting::schema())
        .build()
        .unwrap_err();
    assert_eq!(
        e,
        GraphError::UnroutedSetting {
            stack: "lost",
            key: "nowhere".into(),
        }
    );
}

/// Halves the rate in configure, while its output declares the rate of its input.
struct Halver;

impl Component for Halver {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "halver",
            vec![Port::iq("in")],
            vec![Port::iq("out").same_format_as(0)],
        )
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].with_rate(inputs[0].rate / 2.0)])
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

#[test]
fn a_format_against_the_declared_output_rule_is_a_typed_error() {
    let mut g = GraphBuilder::new()
        .add("h", Halver)
        .add("sink", NullSink::default())
        .input("h")
        .connect("h", "sink")
        .build()
        .unwrap();
    let e = g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap_err();
    assert_eq!(
        e,
        GraphError::OutputFormat {
            component: "h".into(),
            output: "out".into(),
            problem: OutputProblem::Rate {
                declared: 48e3,
                stated: 24e3
            },
        }
    );
}

/// A source of one output port, given at construction.
struct Gives(Port);

impl Component for Gives {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("test.gives", vec![Port::iq("in")], vec![self.0.clone()])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::Bytes, 0.0)])
    }
    fn process(&mut self, _: &mut Io) {}
    fn values(&self) -> Values {
        no_values()
    }
}

#[test]
fn bytes_without_a_sample_type_or_a_bit_order_fail_at_connect() {
    // The standard catalog changes raw samples in bytes to `Iq`. A components crate adds the unpacking of bits.
    let mut r = Registry::standard();
    r.add_adapter(Adapter {
        kind: "test.unpack",
        from: Kind::Bytes,
        to: Kind::HardBits,
        class: AdapterClass::Lossless,
        cost: 1,
        settings: |f, _| f.bit_order.map(|_| no_values()),
        make: || Box::new(Takes(Port::bytes("in"))),
    });
    let connect = |out: Port, want: Port| {
        let mut p = GraphPlan::new();
        p.set_catalog(r.catalog());
        let g = p.add_component("g", Box::new(Gives(out))).unwrap();
        let t = p.add_component("t", Box::new(Takes(want))).unwrap();
        p.connect(PortRef::new(g, 0), PortRef::new(t, 0))
    };
    let iq = || Port::iq("in");
    let bits = || Port::hard_bits("in");
    assert!(matches!(
        connect(Port::bytes("out"), iq()),
        Err(ConnectError::NoConversion { .. })
    ));
    assert!(matches!(
        connect(Port::bytes("out"), bits()),
        Err(ConnectError::NoConversion { .. })
    ));
    assert_eq!(
        connect(Port::bytes("out").sample_type(SampleType::Cs16), iq()),
        Ok(())
    );
    assert_eq!(
        connect(Port::bytes("out").bit_order(BitOrder::MsbFirst), bits()),
        Ok(())
    );
}
