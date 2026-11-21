//! Shared helpers for the tests: feeding a receiver in one piece or in seeded random pieces, reading its saved state, and reading
//! the recordings.

use sdrkit_core::{
    Complex32, Generator, InputInfo, Output, Owner, Receiver, Results, Schema, StackConfig,
    StackDescriptor, StateBlob, Value, Values,
};
use sdrkit_graph::StackGraph;
use serde_json::Value as J;

/// The frequency to tune `desc` to: its default, else the starting dial that a host reads from the data folder.
pub fn start_hz(desc: &StackDescriptor) -> f64 {
    static DIALS: std::sync::OnceLock<Vec<sdrkit_presets::default_dials::DefaultDial>> =
        std::sync::OnceLock::new();
    if desc.tuning.default_hz > 0.0 {
        return desc.tuning.default_hz;
    }
    let rows = DIALS.get_or_init(|| {
        let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data"))
            .expect("the data folder");
        sdrkit_presets::radio_dials::dials(&data).expect("the dials")
    });
    sdrkit_presets::default_dials::dial_hz(rows, desc.id, None)
        .unwrap_or_else(|| panic!("no dial for {}", desc.id))
}

/// The settings of `desc`: its schema's defaults, with `extra` in the settings JSON form.
pub fn settings(desc: &StackDescriptor, extra: &[(&str, J)]) -> Values {
    let mut v = Values::defaults(&(desc.settings)());
    let map: serde_json::Map<String, J> = extra
        .iter()
        .map(|(k, x)| (k.to_string(), x.clone()))
        .collect();
    v.update_from_json(&J::Object(map))
        .expect("settings the stack takes");
    v
}

/// The receiver of `desc` as its stack graph, with its default settings plus `extra`.
pub fn stack_graph(desc: &StackDescriptor, extra: &[(&str, J)]) -> StackGraph {
    let cfg = StackConfig::new(settings(desc, extra));
    let built = match desc.id {
        "cell" => sdrkit_stack_cell::lte::build(&cfg),
        "nr" => sdrkit_stack_cell::nr::build(&cfg),
        "nbiot" => sdrkit_stack_cell::nbiot::build(&cfg),
        "gsmr" => sdrkit_stack_cell::gsm::build(&cfg),
        "lbj" => sdrkit_stack_cell::lbj::build(&cfg),
        "ebicab" => sdrkit_stack_cell::balise::build(&cfg),
        "eot" => sdrkit_stack_cell::eot::build(&cfg),
        "tdscdma" => sdrkit_stack_cell::tdscdma::build(&cfg),
        "cdma2000" => sdrkit_stack_cell::cdma2000::build(&cfg),
        "umts" => sdrkit_stack_cell::umts::build(&cfg),
        "cv2x" => sdrkit_stack_cell::cv2x::build(&cfg),
        other => panic!("no stack {other}"),
    };
    built.expect("the stack graph builds")
}

/// Configures `rx` for `input` and feeds `x` in `pieces` (one piece when empty). Returns the saved state.
pub fn run_graph(rx: &mut StackGraph, input: InputInfo, x: &[Complex32], pieces: &[usize]) -> J {
    rx.configure(&input).expect("configure");
    let mut out = Output::default();
    if pieces.is_empty() {
        rx.feed(x, &mut out);
    } else {
        let mut i = 0;
        let mut k = 0;
        while i < x.len() {
            let n = pieces[k % pieces.len()].min(x.len() - i);
            rx.feed(&x[i..i + n], &mut out);
            out.clear();
            i += n;
            k += 1;
        }
    }
    rx.finish(&mut out);
    state(rx)
}

/// The saved state of `rx` as JSON: `{"components": {name: {"clock", "data"}}}`.
pub fn state(rx: &StackGraph) -> J {
    state_json(&rx.save_state().expect("a saved state"))
}

/// A saved state of a stack graph as JSON: `{"components": {name: {"clock", "data"}}}`.
pub fn state_json(state: &StateBlob) -> J {
    let graph: StateBlob = serde_json::from_slice(state.bytes()).expect("a graph state");
    let parts: Vec<(String, StateBlob)> =
        serde_json::from_slice(graph.bytes()).expect("the parts of a graph state");
    let mut components = serde_json::Map::new();
    for (name, blob) in parts {
        let j: J = serde_json::from_slice(blob.bytes()).expect("a component state is JSON");
        components.insert(name, j);
    }
    serde_json::json!({ "components": components })
}

/// What component `name` holds in a saved state.
pub fn data<'a>(state: &'a J, name: &str) -> &'a J {
    &state["components"][name]["data"]
}

/// The entries of a saved event ring, oldest first.
pub fn entries(ring: &J) -> Vec<J> {
    ring["entries"].as_array().cloned().unwrap_or_default()
}

/// The number value of the published field `key`.
pub fn value(r: &Results, key: &str) -> Option<f64> {
    let id = r.decl().field_id(key)?;
    match r.fields().value(id)? {
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
        Value::I64(x) => Some(*x as f64),
        _ => None,
    }
}

/// The rows of table `key` of `r`.
pub fn table_len(r: &Results, key: &str) -> usize {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// What the search component holds in a saved state: `tables`, `log`, `decoded` and `failed`.
pub fn held(state: &J) -> &J {
    &state["components"]["search"]["data"]
}

/// The cell records of a saved state, most recently seen first, each with its `key` and its `seen`.
pub fn cells(state: &J) -> Vec<J> {
    let rows = held(state)["tables"]["cells"]["rows"]
        .as_array()
        .expect("a table of cells");
    rows.iter()
        .rev()
        .map(|r| {
            let mut c = r["record"].clone();
            c["key"] = r["key"].clone();
            c["seen"] = r["seen"].clone();
            c
        })
        .collect()
}

/// Seeded piece sizes of 1 to 100 000 samples.
pub fn random_pieces(seed: u64, total: usize) -> Vec<usize> {
    let mut s = seed | 1;
    let mut v = Vec::new();
    let mut n = 0;
    while n < total {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        let k = (s % 100_000) as usize + 1;
        v.push(k);
        n += k;
    }
    v
}

/// Runs a fresh receiver of `desc` (default settings plus `extra`) over `x` in `pieces` (one piece when empty). Returns what an
/// old detail view held: `cells` (most recently seen first), `captures`, `retunes`, `decoded`, `failed` and the saved `state`.
pub fn run_mode(
    desc: &StackDescriptor,
    extra: &[(&str, J)],
    input: InputInfo,
    x: &[Complex32],
    pieces: &[usize],
) -> J {
    let mut rx = stack_graph(desc, extra);
    let s = run_graph(&mut rx, input, x, pieces);
    view(&rx, s)
}

/// The view of `rx` and its saved state `s` that [`run_mode`] returns.
pub fn view(rx: &StackGraph, s: J) -> J {
    let r = Owner::results(rx);
    let n = |k: &str| value(r, k).map_or(J::Null, |v| J::from(v as i64));
    serde_json::json!({
        "cells": cells(&s),
        "captures": n("captures"),
        "retunes": n("retunes"),
        "decoded": n("decoded"),
        "failed": n("failed"),
        "state": s,
    })
}

/// `gen` through the standard impairments of `keys` (`gen_*` settings in the settings JSON form), `seconds` long.
pub fn impaired(gen: impl Generator + 'static, keys: &[(&str, J)], seconds: f64) -> Vec<Complex32> {
    let v = gen_values(keys);
    let rate = gen.rate_hz();
    let mut g = sdrkit_components::sim::impaired(Box::new(gen), &v).expect("the impairments");
    let mut x = vec![Complex32::default(); (rate * seconds) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

/// The standard `gen_*` settings with `keys` (in the settings JSON form) and the defaults for the others.
pub fn gen_values(keys: &[(&str, J)]) -> Values {
    let schema = sdrkit_components::sim::with_generator_settings(&Schema::of(&[]))
        .expect("the generator settings");
    let mut v = Values::defaults(&schema);
    let map: serde_json::Map<String, J> = keys
        .iter()
        .map(|(k, x)| (k.to_string(), x.clone()))
        .collect();
    v.update_from_json(&J::Object(map))
        .expect("generator settings");
    v
}

/// Noise for `snr_db` in the signal's band (the `gen_*` keys for [`impaired`]).
pub fn snr(db: f64) -> Vec<(&'static str, J)> {
    vec![("gen_noise", J::from("snr")), ("gen_snr_db", J::from(db))]
}

/// A recording and its section of expected.json; `None` (the reason printed) when the recording is missing.
pub fn vector(name: &str) -> Option<(sdrkit_dev_test::sigmf::Recording, J)> {
    let rec = sdrkit_dev_test::read_recording(&format!("cell/{name}"))?;
    let exp = sdrkit_dev_test::sigmf::read_expected("cell").expect("expected.json");
    Some((rec, exp["recordings"][name].clone()))
}

/// |a - b| <= tol for JSON numbers.
pub fn near(a: &J, b: f64, tol: f64) -> bool {
    a.as_f64().is_some_and(|x| (x - b).abs() <= tol)
}

/// Removes the fields that depend on the wall clock, so two runs can be compared.
pub fn without_wall(mut j: J) -> J {
    fn walk(j: &mut J) {
        match j {
            J::Object(m) => {
                m.remove("wall");
                m.remove("first_seen_wall");
                m.remove("last_seen_wall");
                m.remove("first_wall");
                m.remove("last_wall");
                for v in m.values_mut() {
                    walk(v);
                }
            }
            J::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut j);
    j
}

/// The first path where `a` and `b` differ, with both values, or `None` when they are the same.
pub fn first_difference(a: &J, b: &J, path: &str) -> Option<String> {
    match (a, b) {
        (J::Object(x), J::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                let (p, q) = (x.get(k).unwrap_or(&J::Null), y.get(k).unwrap_or(&J::Null));
                if let Some(d) = first_difference(p, q, &format!("{path}.{k}")) {
                    return Some(d);
                }
            }
            None
        }
        (J::Array(x), J::Array(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .enumerate()
            .find_map(|(i, (p, q))| first_difference(p, q, &format!("{path}[{i}]"))),
        _ if a == b => None,
        _ => Some(format!("{path}: {a} against {b}")),
    }
}

/// Asserts that `a` and `b` are the same apart from wall-clock times, naming the first difference.
pub fn assert_same(a: &J, b: &J) {
    let (a, b) = (without_wall(a.clone()), without_wall(b.clone()));
    if let Some(d) = first_difference(&a, &b, "") {
        panic!("not the same: {d}");
    }
}

/// Runs the `clear` command of `rx`.
pub fn clear<O: Owner + ?Sized>(rx: &mut O) {
    let id = rx.commands().id("clear").expect("a clear command");
    let args = rx.commands().defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(rx, id, &args).expect("clear runs");
}

/// The text of table `table` column `column` in row `row` of `r`, for people.
pub fn cell_text(r: &Results, table: &str, row: &str, column: &str) -> Option<String> {
    let t = r.table(r.decl().table_id(table)?)?;
    let rec = t.row(row)?;
    let id = rec.decl().id(column)?;
    Some(rec.text(id)?.to_string())
}

/// The text of the data file of the locomotive classes, `data/cell/locomotives.txt` of the repository.
pub fn locomotives_text() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/cell/locomotives.txt"))
        .expect("the data file of the locomotive classes")
}

/// A plan file of the repository's `data/plans/` folder, read as the host's plan import reads it.
pub fn plan_file(name: &str) -> sdrkit_data_plans::data::PlanFile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/plans")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    let mut plans = sdrkit_data_plans::Plans::new();
    sdrkit_data_plans::api::import_as(
        &mut plans,
        &text,
        sdrkit_data_plans::api::PlanFormat::Json,
        "",
        "",
    )
    .expect("the registry takes the file");
    sdrkit_data_plans::data::parse(name, &text).expect("a plan file")
}

/// The values of the row (`stack`, `list`, `name`) of the repository's `data/cell/band_order.csv`, apart by spaces.
pub fn band_row(stack: &str, list: &str, name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data/cell/band_order.csv");
    let text = std::fs::read_to_string(&path).expect("the data file");
    let key = format!("{stack},{list},{name},");
    text.lines()
        .find_map(|l| l.strip_prefix(key.as_str()))
        .unwrap_or_else(|| panic!("no row {key}"))
        .trim()
        .to_owned()
}
