//! Setting helpers shared by the component tests: write a value as text or `F64` and get the typed value of the setting.

#![allow(dead_code)]

use sdrkit_core::{Complex32, Results, Schema, SettingError, Value, ValueKind};
use sdrkit_graph::stream::Data;
use sdrkit_graph::{Applied, Component, Format, Graph, GraphBuilder, GraphError};
use std::collections::HashMap;
use std::sync::Arc;

/// The value of setting `key` of `schema` written as `v`: a choice key given as text becomes its index, and a number given as
/// `F64` takes the kind of the setting.
pub fn typed(schema: &Schema, key: &str, v: &Value) -> Value {
    let id = schema.id(key).unwrap_or_else(|e| panic!("{key}: {e}"));
    let field = &schema.setting(id).unwrap().field;
    match (field.kind, v) {
        (ValueKind::Choice, Value::Text(t)) => {
            Value::Choice(field.choice_index(t).unwrap_or(u16::MAX))
        }
        (ValueKind::I64, Value::F64(x)) if x.fract() == 0.0 => Value::I64(*x as i64),
        (ValueKind::Hz, Value::F64(x)) => Value::Hz(*x),
        _ => v.clone(),
    }
}

/// Applies `v` (written as [`typed`] takes it) to setting `key` of `c`, through the schema check.
pub fn set(c: &mut dyn Component, key: &str, v: &Value) -> Result<Applied, SettingError> {
    let schema = c.info().settings;
    let id = schema.id(key)?;
    let v = typed(&schema, key, v);
    schema.check(id, &v)?;
    c.apply(id, &v)
}

/// Applies `v` to setting `key` of the running graph `g`, through the schema check.
pub fn gset(g: &mut Graph, key: &str, v: &Value) -> Result<(), SettingError> {
    use sdrkit_core::Owner;
    let schema = g.schema();
    let id = schema.id(key)?;
    let v = typed(schema, key, v);
    sdrkit_core::owner::apply(g, id, &v)
}

/// A results field of component `id`.
pub fn result(g: &Graph, id: &str, key: &str) -> Value {
    let r: &Results = g
        .component_results(id)
        .unwrap_or_else(|| panic!("{id} has no results"));
    let f = r.fields();
    match f.decl().id(key).and_then(|i| f.value(i)) {
        Some(v) => v.clone(),
        None => panic!("{id} has no result {key}"),
    }
}

/// A results field of component `id`, as a number.
pub fn result_f64(g: &Graph, id: &str, key: &str) -> f64 {
    match result(g, id, key) {
        Value::F64(v) | Value::Hz(v) => v,
        Value::I64(v) => v as f64,
        other => panic!("{id} result {key} is not a number: {other:?}"),
    }
}

/// The typed value of setting `key` of component `c`, with `v` written as [`typed`] takes it.
pub fn tv(c: &dyn Component, key: &str, v: Value) -> Value {
    typed(&c.info().settings, key, &v)
}

/// Text as a value.
pub fn txt(s: &str) -> Value {
    Value::Text(s.into())
}

/// A value as a test writes it: text, a number or a flag, or a [`Value`]. [`typed`] converts it to the kind of its setting.
pub struct Written(pub Value);

impl From<Value> for Written {
    fn from(v: Value) -> Self {
        Written(v)
    }
}
impl From<&str> for Written {
    fn from(v: &str) -> Self {
        Written(Value::Text(v.into()))
    }
}
impl From<f64> for Written {
    fn from(v: f64) -> Self {
        Written(Value::F64(v))
    }
}
impl From<bool> for Written {
    fn from(v: bool) -> Self {
        Written(Value::Bool(v))
    }
}
impl From<i64> for Written {
    fn from(v: i64) -> Self {
        Written(Value::I64(v))
    }
}

/// A [`GraphBuilder`] whose `set` takes a value as [`typed`] takes it.
#[derive(Default)]
pub struct Tb {
    b: GraphBuilder,
    schemas: HashMap<String, Arc<Schema>>,
}

impl Tb {
    pub fn new() -> Self {
        Tb::default()
    }
    pub fn add(self, id: &str, c: impl Component + 'static) -> Self {
        self.add_boxed(id, Box::new(c))
    }
    pub fn add_boxed(mut self, id: &str, c: Box<dyn Component>) -> Self {
        self.schemas.insert(id.to_string(), c.info().settings);
        self.b = self.b.add_boxed(id, c);
        self
    }
    pub fn connect(mut self, from: &str, to: &str) -> Self {
        self.b = self.b.connect(from, to);
        self
    }
    pub fn input(mut self, to: &str) -> Self {
        self.b = self.b.input(to);
        self
    }
    pub fn output(mut self, from: &str) -> Self {
        self.b = self.b.output(from);
        self
    }
    /// Sets `"<component>.<key>"`.
    pub fn set(mut self, key: &str, v: impl Into<Written>) -> Self {
        let v = v.into().0;
        let typed_value = match key.split_once('.') {
            Some((id, k)) => match self.schemas.get(id) {
                Some(schema) if schema.id(k).is_ok() => typed(schema, k, &v),
                _ => v,
            },
            None => v,
        };
        self.b = self.b.set(key, typed_value);
        self
    }
    pub fn build(self) -> Result<Graph, GraphError> {
        self.b.build()
    }
    /// Ends the graph in a `null_sink` after `from` (a component or `"<component>.<port>"`), so that the chain runs, and builds it.
    pub fn finish(self, from: &str) -> Graph {
        let sink = sdrkit_components::registry().make("dsp.null_sink").unwrap();
        self.add_boxed("sink", sink)
            .connect(from, "sink")
            .build()
            .unwrap()
    }
}

/// Ends `b` in a `null_sink` after `from` and builds it (see [`Tb::finish`]).
pub fn finish(b: Tb, from: &str) -> Graph {
    b.finish(from)
}

/// [`sdrkit_components::testing::run_iq`] with a tap open on `port`, so that the graph runs the component.
pub fn run_iq(g: &mut Graph, input: Format, x: &[Complex32], piece: usize, port: &str) -> Data {
    let _tap = g.tap(port, 1).unwrap();
    sdrkit_components::testing::run_iq(g, input, x, piece, port)
}

/// [`sdrkit_components::testing::run_passes`] with a tap open on `port`.
pub fn run_passes(g: &mut Graph, passes: usize, port: &str) -> Data {
    let _tap = g.tap(port, 1).unwrap();
    sdrkit_components::testing::run_passes(g, passes, port)
}
