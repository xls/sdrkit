//! Typed settings, aliases, typed item ports and description errors.

use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::field;
use sdrkit_core::{ConfigError, Setting, SettingError, SettingId, SettingSet, Value};
use sdrkit_graph::{
    Applied, Component, ComponentInfo, ConnectError, DescPosition, Description, Format,
    GraphBuilder, GraphError, GraphPlan, Io, Item, Kind, KindRule, Port, PortRef, Registry,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const GAIN: Setting<field::F64> = Setting::new("gain", "Gain", "", 1.0).range(0.0, 10.0, 0.0);

sdrkit_core::settings! {
    /// The settings of the test component.
    enum GainSetting {
        Gain = GAIN,
    }
}

/// Multiplies a real signal by a gain, and counts the calls of `apply`.
struct Gain {
    gain: f64,
    applied: Arc<AtomicUsize>,
}

impl Gain {
    fn new(applied: &Arc<AtomicUsize>) -> Gain {
        Gain {
            gain: 1.0,
            applied: Arc::clone(applied),
        }
    }
}

impl Component for Gain {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::values_of([(GainSetting::Gain, Value::F64(self.gain))])
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor("gain", vec![Port::iq("in")], vec![Port::iq("out")])
            .with_settings(GainSetting::schema())
    }
    fn configure(&mut self, inputs: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![inputs[0].clone()])
    }
    fn process(&mut self, io: &mut Io) {
        let g = self.gain as f32;
        for s in io.pass_through::<sdrkit_core::Complex32>(0, 0) {
            *s *= g;
        }
    }
    fn apply(&mut self, id: SettingId, v: &Value) -> Result<Applied, SettingError> {
        self.applied.fetch_add(1, Ordering::Relaxed);
        match GainSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            GainSetting::Gain => self.gain = GAIN.read(id, v)?,
        }
        Ok(Applied::Live)
    }
}

#[test]
fn a_value_that_the_schema_refuses_never_reaches_apply() {
    let applied = Arc::new(AtomicUsize::new(0));
    let mut g = GraphBuilder::new()
        .add("a", Gain::new(&applied))
        .input("a")
        .build()
        .unwrap();
    let id = g.schema().id("a.gain").unwrap();
    assert_eq!(
        owner::apply(&mut g, id, &Value::F64(20.0)),
        Err(SettingError::OutOfRange {
            id,
            min: 0.0,
            max: 10.0,
            got: 20.0
        })
    );
    assert!(matches!(
        owner::apply(&mut g, id, &Value::Bool(true)),
        Err(SettingError::WrongKind { .. })
    ));
    assert_eq!(
        owner::apply(&mut g, id, &Value::F64(f64::NAN)),
        Err(SettingError::NotFinite { id })
    );
    assert_eq!(applied.load(Ordering::Relaxed), 0);
    owner::apply(&mut g, id, &Value::F64(2.0)).unwrap();
    assert_eq!(applied.load(Ordering::Relaxed), 1);

    let refused = GraphBuilder::new()
        .add("a", Gain::new(&applied))
        .input("a")
        .set("a.gain", Value::F64(-1.0))
        .build();
    assert!(matches!(
        refused,
        Err(GraphError::Setting {
            error: SettingError::OutOfRange { .. },
            ..
        })
    ));
    assert_eq!(applied.load(Ordering::Relaxed), 1);
}

#[test]
fn an_alias_resolves_to_its_component_and_id_at_build() {
    let applied = Arc::new(AtomicUsize::new(0));
    let g = GraphBuilder::new()
        .add("a", Gain::new(&applied))
        .input("a")
        .alias("level", "a.gain")
        .build()
        .unwrap();
    let t = g.target("level").unwrap();
    assert_eq!(g.target_component(t), "a");
    assert_eq!(t.id(), GainSetting::Gain.id());
    assert_eq!(g.schema().len(), 1);
    assert!(g.schema().id("level").is_ok());

    let unknown = GraphBuilder::new()
        .add("a", Gain::new(&applied))
        .input("a")
        .alias("level", "a.volume")
        .build();
    assert_eq!(
        unknown.err(),
        Some(GraphError::Setting {
            key: "level".into(),
            error: SettingError::Unknown {
                key: "a.volume".into()
            },
        })
    );
}

#[derive(Clone, Debug)]
struct Frame;
impl Item for Frame {
    const KIND: &'static str = "test.frame";
}

#[derive(Clone, Debug)]
struct Picture;
impl Item for Picture {
    const KIND: &'static str = "test.picture";
}

/// Makes typed items of `T`.
struct Source<T>(std::marker::PhantomData<T>);
impl<T: Item> Component for Source<T> {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::processor(
            "source",
            vec![Port::iq("in")],
            vec![Port::typed::<T>("out")],
        )
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![Format::new(Kind::typed::<T>(), 0.0)])
    }
    fn process(&mut self, _: &mut Io) {}
}

/// Takes typed items of `T`.
struct Sink<T>(std::marker::PhantomData<T>);
impl<T: Item> Component for Sink<T> {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("sink", vec![Port::typed::<T>("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {}
}

#[test]
fn a_typed_item_port_refuses_another_item_type_at_connect() {
    let mut p = GraphPlan::new();
    let src = p
        .add_component("src", Box::new(Source::<Frame>(Default::default())))
        .unwrap();
    let pic = p
        .add_component("pic", Box::new(Sink::<Picture>(Default::default())))
        .unwrap();
    let frm = p
        .add_component("frm", Box::new(Sink::<Frame>(Default::default())))
        .unwrap();
    p.input(PortRef::new(src, 0)).unwrap();
    match p.connect(PortRef::new(src, 0), PortRef::new(pic, 0)) {
        Err(ConnectError::ItemTypeMismatch {
            produces, accepts, ..
        }) => {
            assert_eq!(Kind::Item(produces), Kind::typed::<Frame>());
            assert_eq!(accepts, KindRule::Is(Kind::typed::<Picture>()));
        }
        other => panic!("{other:?}"),
    }
    p.connect(PortRef::new(src, 0), PortRef::new(frm, 0))
        .unwrap();
}

#[test]
fn a_description_error_names_its_position() {
    let r = Registry::standard();
    let unknown_kind = r#"{
        "components": [
            { "id": "chan", "type": "dsp.channel" },
            { "id": "x", "type": "no_such_kind" }
        ]
    }"#;
    let e = Description::from_json(unknown_kind)
        .unwrap()
        .build(&r)
        .unwrap_err();
    assert_eq!(
        e,
        GraphError::Description {
            at: DescPosition::Component { index: 1 },
            error: Box::new(GraphError::UnknownType("no_such_kind".into())),
        }
    );

    let bad_setting = r#"{
        "components": [{ "id": "chan", "type": "dsp.channel", "settings": { "rate": "fast" } }]
    }"#;
    match Description::from_json(bad_setting).unwrap().build(&r) {
        Err(GraphError::Description { at, error }) => {
            assert_eq!(
                at,
                DescPosition::ComponentSetting {
                    index: 0,
                    key: "rate".into()
                }
            );
            assert!(matches!(*error, GraphError::SettingValue { .. }));
        }
        other => panic!("{other:?}"),
    }

    let out_of_range = r#"{
        "components": [{ "id": "chan", "type": "dsp.channel", "settings": { "rate": -5 } }]
    }"#;
    match Description::from_json(out_of_range).unwrap().build(&r) {
        Err(GraphError::Description { error, .. }) => assert!(matches!(
            *error,
            GraphError::SettingValue { .. } | GraphError::Setting { .. }
        )),
        other => panic!("{other:?}"),
    }

    let unknown_entry = "{\n  \"components\": [],\n  \"colour\": 1\n}";
    assert!(matches!(
        Description::from_json(unknown_entry),
        Err(GraphError::Syntax { line: 3, .. })
    ));
}
