use sdrkit_algo_dsp::Frame;
use sdrkit_components::testing::run_passes;
use sdrkit_core::SettingError;
use sdrkit_core::{Choice, ConfigError, SettingId, Value, Values};
use sdrkit_graph::stream::{Data, Timed};
use sdrkit_graph::{
    no_values, Applied, Component, ComponentInfo, Format, GraphBuilder, Io, Kind, Port, Registry,
};
use sdrkit_proto_bcdata::component::{Pcm, PlayoutComponent};
use sdrkit_proto_bcdata::playout::Concealment;
use std::sync::Arc;

/// Emits one 100 ms Pcm item per pass, with signal times. Every fifth is lost.
struct PcmSource {
    n: usize,
}

impl Component for PcmSource {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::generator("test.pcm", vec![Port::typed::<Pcm>("out")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<Pcm>(), 10.0)])
    }
    fn process(&mut self, io: &mut Io) {
        let k = self.n;
        self.n += 1;
        let value = if k % 5 == 4 {
            Pcm::Lost(4800)
        } else {
            Pcm::Sound(Arc::from(vec![Frame([0.2, 0.2]); 4800]))
        };
        io.items_out::<Pcm>(0).push(Timed {
            time: k as f64 * 0.1,
            value,
        });
    }
    fn apply(&mut self, id: SettingId, _: &Value) -> Result<Applied, SettingError> {
        Err(SettingError::NoSuchId { id })
    }
    fn values(&self) -> Values {
        no_values()
    }
}

#[test]
fn playout_component_releases_by_signal_time() {
    let mut r = Registry::new();
    sdrkit_proto_bcdata::register(&mut r);
    let mut c = r.make("bcdata.playout").unwrap();
    assert_eq!(c.info().name, "bcdata.playout");
    let schema = c.info().settings;
    let id = schema.id("concealment").unwrap();
    let repeat = Value::Choice(Concealment::Repeat.index());
    schema.check(id, &repeat).unwrap();
    c.apply(id, &repeat).unwrap();
    assert_eq!(c.values().value(id), Some(&repeat));
    assert!(schema.check(id, &Value::Choice(9)).is_err());
    assert!(schema.id("other").is_err());
    assert!(PlayoutComponent::new(1_000).is_err());
    let mut g = GraphBuilder::new()
        .add("src", PcmSource { n: 0 })
        .add("play", PlayoutComponent::new(48_000).unwrap())
        .add("sink", sdrkit_graph::components::NullSink::default())
        .connect("src", "play")
        .connect("play", "sink")
        .build()
        .unwrap();
    let out = run_passes(&mut g, 41, "play.out");
    let Data::Audio(a) = out else { panic!("audio") };
    // 40 items after the first: 4 s at 48 kHz.
    assert_eq!((a.channels, a.frames()), (2, 4 * 48_000));
    assert!(a.stereo().iter().any(|f| f[0] != 0.0));
}

#[test]
fn a_change_of_frequency_clears_the_playout_counts() {
    use sdrkit_core::interface::ReadFields;
    use sdrkit_graph::Kept;
    use sdrkit_proto_bcdata::component::PlayoutFields;
    let mut g = GraphBuilder::new()
        .add("src", PcmSource { n: 0 })
        .add("play", PlayoutComponent::new(48_000).unwrap())
        .add("sink", sdrkit_graph::components::NullSink::default())
        .connect("src", "play")
        .connect("play", "sink")
        .build()
        .unwrap();
    let counts = |g: &sdrkit_graph::Graph| {
        PlayoutFields::read(g.component_results("play").unwrap().fields()).unwrap()
    };
    run_passes(&mut g, 41, "play.out");
    let before = counts(&g);
    assert!(before.concealments.is_some_and(|n| n > 0), "{before:?}");
    assert!(before.concealed.is_some());
    g.component_mut("play").unwrap().forget(&Kept::of(&[], &[]));
    let after = counts(&g);
    assert_eq!(
        after,
        PlayoutFields {
            concealed: None,
            muted: None,
            concealments: None,
            underruns: None,
        }
    );
    // The next report counts from zero.
    run_passes(&mut g, 21, "play.out");
    let again = counts(&g);
    assert!(
        again
            .concealments
            .is_some_and(|n| n < before.concealments.unwrap()),
        "{again:?}"
    );
}
