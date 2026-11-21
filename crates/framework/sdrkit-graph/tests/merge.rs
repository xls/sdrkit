//! The merge of results: the declaration exists at build, the graph's own tables keep their rows, and a copied series keeps
//! the room of its source.

use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::{ConfigError, InputInfo, Output, Owner, Receiver, Results, ResultsDecl, Values};
use sdrkit_graph::*;
use std::sync::Arc;

const LEVEL: Field<field::F64> = Field::new("level", "Level", "dB");
const NAME: Field<field::Text> = Field::new("name", "Name", "");
const PRESETS: TableDef = TableDef {
    key: "presets",
    label: "Presets",
    columns: &[&NAME],
    cap: 16,
};

/// A sink that publishes a level and a trace of 1000 points with room for 4096.
struct Meter(Published);

impl Meter {
    fn new() -> Meter {
        let decl = ResultsDecl::new()
            .with_fields(&[&LEVEL])
            .series("trace", "Trace", "s", "dB");
        Meter(Published::from_decl(Arc::new(decl)))
    }
}

impl Component for Meter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.meter", vec![Port::iq("in")])
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {
        self.0.set(&LEVEL, -20.0);
        let r = self.0.results_mut();
        let id = r.decl().series_id("trace").unwrap();
        r.update_series(id, |s| {
            s.points.clear();
            s.points.reserve(4096);
            s.points.extend((0..1000).map(|i| [i as f32, 0.0]));
            true
        });
    }
    fn results(&self) -> Option<&Results> {
        Some(self.0.results())
    }
}

#[test]
fn a_stack_graph_declares_its_results_at_build() {
    let g = GraphBuilder::new()
        .add("meter", Meter::new())
        .input("meter")
        .build()
        .unwrap();
    let rx = StackGraph::builder("test", g).build().unwrap();
    assert!(Owner::results_decl(&rx).field_id("meter.level").is_some());
}

#[test]
fn the_graph_own_tables_exist_before_configure_and_keep_their_rows() {
    let own = Published::new(&[]).with_tables(&[&PRESETS]);
    let mut g = GraphBuilder::new()
        .add("meter", Meter::new())
        .input("meter")
        .publish(own)
        .build()
        .unwrap();
    let table = Owner::results_decl(&g)
        .table_id("presets")
        .expect("declared at build");
    g.own_results_mut()
        .unwrap()
        .row(&PRESETS, "1", vec![sdrkit_core::Value::Text("home".into())]);
    g.configure(&InputInfo::new(48e3, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    for _ in 0..3 {
        g.process(&[Default::default(); 4800], &mut out);
    }
    let rows = Owner::results(&g).table(table).unwrap();
    assert_eq!(rows.len(), 1);
    // A copied series has the room of its source, so it does not grow while the source fills.
    let r = Owner::results(&g);
    let trace = r.decl().series_id("meter.trace").unwrap();
    assert!(r.series(trace).unwrap().points.capacity() >= 4096);
}
