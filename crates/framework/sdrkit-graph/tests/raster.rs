//! Rasters, pictures that are results, are merged from a graph's components like series: a declared key keeps its name, and
//! another key is prefixed with its component.

use sdrkit_core::results::copy_if_changed;
use sdrkit_core::{Axis, ConfigError, Output, Raster, Results, ResultsDecl};
use sdrkit_graph::{Component, ComponentInfo, Format, GraphBuilder, Io, Port};
use std::sync::Arc;

struct Map {
    results: Results,
}

impl Map {
    fn new() -> Map {
        let decl = ResultsDecl::new()
            .raster("lv", "l-v map", "deg", "km/s", "K")
            .raster("extra", "Extra", "deg", "km/s", "K");
        Map {
            results: Results::new(Arc::new(decl)),
        }
    }
}

impl Component for Map {
    fn values(&self) -> sdrkit_core::Values {
        sdrkit_graph::no_values()
    }
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("map", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, io: &mut Io) {
        let n = io.iq(0).len();
        let mut r = Raster::new(
            4,
            2,
            Axis::new(0.0, 0.5, "MHz"),
            Axis::new(0.0, 15.0, "km"),
            "dB",
        );
        r.set(1, 3, n as f32);
        let decl = Arc::clone(self.results.decl());
        let lv = decl.raster_id("lv").unwrap();
        let extra = decl.raster_id("extra").unwrap();
        self.results.update_raster(lv, |d| copy_if_changed(d, &r));
        self.results
            .update_raster(extra, |d| copy_if_changed(d, &r));
    }
    fn results(&self) -> Option<&Results> {
        Some(&self.results)
    }
}

#[test]
fn rasters_are_merged_under_their_declared_keys() {
    let decl = ResultsDecl::new().raster("lv", "l-v map", "deg", "km/s", "K");
    let mut g = GraphBuilder::new()
        .add("a", Map::new())
        .input("a")
        .declare(&decl)
        .build()
        .unwrap();
    g.configure_input(Format::iq(48e3, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    g.process(&[Default::default(); 100], &mut out);
    g.refresh_results();
    let r = sdrkit_core::Owner::results(&g);
    let lv = r.raster(r.decl().raster_id("lv").unwrap()).unwrap();
    assert_eq!((lv.width, lv.height), (4, 2));
    assert_eq!(lv.get(1, 3), Some(100.0));
    assert!(lv.get(0, 0).unwrap().is_nan());
    assert_eq!(lv.y.at(1), 15.0);
    assert!(r.decl().raster_id("a.extra").is_some());
    let lv_id = decl.raster_id("lv").unwrap();
    assert_eq!(decl.rasters()[lv_id.0 as usize].unit, "K");
    // The cells stay out of the JSON form for people.
    let json = serde_json::to_string(r).unwrap();
    assert!(json.contains("\"width\":4") && !json.contains("values"));
}
