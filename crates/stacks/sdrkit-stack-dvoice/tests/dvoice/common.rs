//! Helpers shared by the tests: receivers and test signals by stack id, a run over a signal, and a view of the typed results
//! in the shape that the tests and the expected values of the recordings compare (calls oldest first, with their details).
#![allow(dead_code)]

use sdrkit_algo_sim::chain::{NoiseKind, GEN_CLOCK_PPM, GEN_NOISE, GEN_OFFSET_HZ, GEN_SNR_DB};
use sdrkit_core::codec::CodecSet;
use sdrkit_core::{
    Complex32, GenConfig, Generator, InputInfo, Output, Owner, Receiver, Results, StackConfig,
    StackDescriptor, Value, ValueKind, Values,
};
use serde_json::{json, Map, Value as Json};

/// The frequency to tune `d` to: its default, else the starting dial that a host reads from the data folder.
pub fn start_hz(d: &StackDescriptor) -> f64 {
    static DIALS: std::sync::OnceLock<Vec<sdrkit_presets::default_dials::DefaultDial>> =
        std::sync::OnceLock::new();
    if d.tuning.default_hz > 0.0 {
        return d.tuning.default_hz;
    }
    let rows = DIALS.get_or_init(|| {
        let data = sdrkit_presets::Data::open(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data"))
            .expect("the data folder");
        sdrkit_presets::radio_dials::dials(&data).expect("the dials")
    });
    sdrkit_presets::default_dials::dial_hz(rows, d.id, None).unwrap_or_else(|| panic!("no dial for {}", d.id))
}

/// The descriptor of a stack id.
pub fn desc(id: &str) -> &'static StackDescriptor {
    sdrkit_stack_dvoice::descriptors()
        .into_iter()
        .find(|d| d.id == id)
        .expect("a digital voice stack")
}

/// The settings of stack `id` at their defaults, with `extra` given as text ("0" and "1" for yes and no).
pub fn values(id: &str, extra: &[(&str, &str)]) -> Values {
    let schema = (desc(id).settings)();
    let mut v = Values::defaults(&schema);
    let mut j = Map::new();
    for (k, text) in extra {
        let s = schema
            .setting(schema.id(k).expect("a setting of the stack"))
            .unwrap();
        let x = match s.field.kind {
            ValueKind::Bool => json!(matches!(*text, "1" | "true")),
            ValueKind::F64 | ValueKind::Hz | ValueKind::Duration => {
                json!(text.parse::<f64>().expect("a number"))
            }
            ValueKind::I64 => json!(text.parse::<i64>().expect("a whole number")),
            _ => json!(text),
        };
        j.insert((*k).into(), x);
    }
    v.update_from_json(&Json::Object(j)).expect("settings");
    v
}

fn config(id: &str, extra: &[(&str, &str)]) -> StackConfig {
    StackConfig::new(values(id, extra))
}

/// A receiver with the defaults of the stack and `extra` settings.
pub fn receiver(id: &str, extra: &[(&str, &str)]) -> Box<dyn Receiver> {
    (desc(id).make_receiver)(&config(id, extra)).expect("the receiver builds")
}

/// A receiver with codec plug-ins.
pub fn receiver_with_codecs(
    id: &str,
    extra: &[(&str, &str)],
    codecs: CodecSet,
) -> Box<dyn Receiver> {
    let mut g = sdrkit_stack_dvoice::receiver_graph(id, &config(id, extra))
        .expect("a digital voice stack")
        .expect("the graph builds");
    g.set_codecs(codecs);
    Box::new(g)
}

/// One pass of the test signal of the stack at `rate`.
pub fn generator(id: &str, rate: f64, seed: u64, extra: &[(&str, &str)]) -> Box<dyn Generator> {
    let cfg = GenConfig::new(rate, values(id, extra)).with_seed(seed);
    Box::new(sdrkit_stack_dvoice::test_signal(id, &cfg).expect("a digital voice stack"))
}

/// All the samples of a generator.
pub fn generate_all(g: &mut dyn Generator) -> Vec<Complex32> {
    let mut out = Vec::new();
    let mut buf = vec![Complex32::new(0.0, 0.0); 65536];
    loop {
        let n = g.generate(&mut buf);
        out.extend_from_slice(&buf[..n]);
        if n < buf.len() {
            break;
        }
    }
    out
}

/// Feeds samples in blocks of `block`, then 2 s of zeros. It returns the view of the results ([`view`]) and the output.
pub fn run(rx: &mut dyn Receiver, rate: f64, x: &[Complex32], block: usize) -> (Json, Output) {
    rx.configure(&InputInfo::new(rate, 0.0, 0.0).recording())
        .expect("configure");
    let mut all = Output::default();
    let mut out = Output::default();
    let zeros = vec![Complex32::new(0.0, 0.0); (2.0 * rate) as usize];
    for c in x.chunks(block).chain(zeros.chunks(block)) {
        rx.feed(c, &mut out);
        for p in out.audio_runs() {
            all.push_audio(p.samples, p.rate_hz, p.channels);
        }
        all.events.append(&mut out.events);
        out.clear();
    }
    (view(Owner::results(rx)), all)
}

/// A typed value as JSON: a choice as its key, a position as `[lat, lon]`.
pub fn cell(v: &Value, choices: &[sdrkit_core::ChoiceDecl]) -> Json {
    match v {
        Value::Text(t) => json!(t),
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => json!(x),
        Value::I64(x) => json!(x),
        Value::Bool(b) => json!(b),
        Value::Choice(i) => choices
            .get(usize::from(*i))
            .map_or(Json::Null, |c| json!(c.key)),
        Value::Position { lat, lon } => json!([lat, lon]),
        Value::None | Value::Bytes(_) => Json::Null,
    }
}

/// An identity cell as the records carry it: a number for a radio ID or talkgroup, text for a callsign, null when absent.
fn ident(m: &mut Map<String, Json>) {
    for k in ["src", "dst"] {
        let Some(Json::String(t)) = m.get(k).cloned() else {
            continue;
        };
        let v = if t.is_empty() {
            Json::Null
        } else {
            match t.parse::<u64>() {
                Ok(n) if !t.starts_with('0') || t == "0" => json!(n),
                _ => json!(t),
            }
        };
        m.insert(k.into(), v);
    }
}

/// The rows of the table `key` with their row keys, newest first, each row as an object keyed by the declared column keys.
pub fn rows(r: &Results, key: &str) -> Vec<(String, Map<String, Json>)> {
    let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first()
        .map(|(k, row)| {
            let m = row
                .iter()
                .map(|(d, v)| (d.key.to_string(), cell(v, &d.choices)))
                .collect();
            (k.to_owned(), m)
        })
        .collect()
}

/// The rows of a declared table, newest first.
pub fn table(r: &Results, decl: &sdrkit_core::publish::TableDef) -> Vec<Map<String, Json>> {
    rows(r, decl.key).into_iter().map(|(_, m)| m).collect()
}

/// The field `key` as JSON, null when it has no value.
pub fn field(r: &Results, key: &str) -> Json {
    let f = r.fields();
    let Some(id) = r.decl().field_id(key) else {
        return Json::Null;
    };
    let choices = f
        .decl()
        .field(id)
        .map(|d| d.choices.to_vec())
        .unwrap_or_default();
    f.value(id).map_or(Json::Null, |v| cell(v, &choices))
}

/// The view of the typed results that the tests compare: `fields` (every field), `status`, `tables` (the tables of the
/// stacks' own), and `detail` with the calls (each with its `details`), messages, positions, talkgroups, the fields and the
/// site, and the shared measurements.
pub fn view(r: &Results) -> Json {
    use sdrkit_stack_dvoice::results as res;
    let details = rows(r, res::CALL_DETAILS.key);
    let mut calls = Vec::new();
    for (key, mut c) in rows(r, res::CALLS_300.key) {
        let mut d = Map::new();
        for (_, row) in &details {
            if row.get("call").and_then(Json::as_str) == Some(key.as_str()) {
                if let Some(k) = row.get("detail").and_then(Json::as_str) {
                    d.insert(k.into(), row.get("value").cloned().unwrap_or(Json::Null));
                }
            }
        }
        c.insert("details".into(), Json::Object(d));
        let state = c.get("encryption").cloned().unwrap_or(Json::Null);
        let alg = c.get("algorithm").cloned().unwrap_or(Json::Null);
        c.insert("encryption".into(), json!({"state": state, "alg": alg}));
        ident(&mut c);
        let start = c.get("start_s").and_then(Json::as_f64).unwrap_or(0.0);
        let dur = c.get("duration").and_then(Json::as_f64).unwrap_or(0.0);
        c.insert("duration_s".into(), json!(dur));
        c.insert("end_s".into(), json!(start + dur));
        calls.push(Json::Object(c));
    }
    let messages: Vec<Json> = table(r, &res::MESSAGES)
        .into_iter()
        .map(|mut m| {
            ident(&mut m);
            let t = m.remove("time_s").unwrap_or(Json::Null);
            m.insert("time".into(), t);
            Json::Object(m)
        })
        .collect();
    let positions: Vec<Json> = table(r, &res::POSITIONS)
        .into_iter()
        .map(|mut p| {
            ident(&mut p);
            if let Some(Json::Array(ll)) = p.get("position").cloned() {
                p.insert("lat".into(), ll[0].clone());
                p.insert("lon".into(), ll[1].clone());
            }
            Json::Object(p)
        })
        .collect();
    let talkgroups: Vec<Json> = table(r, &res::TALKGROUPS)
        .into_iter()
        .map(|mut t| {
            let tg = t.remove("talkgroup").unwrap_or(Json::Null);
            t.insert("tg".into(), tg);
            Json::Object(t)
        })
        .collect();
    let mut fields = Map::new();
    for (d, v) in r.fields().iter() {
        fields.insert(d.key.to_string(), cell(v, &d.choices));
    }
    let mut protocol = fields.clone();
    if let Some(s) = table(r, &res::SITES).first() {
        let site: Map<String, Json> = s
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        protocol.insert("site".into(), Json::Object(site));
    }
    let mut tables = Map::new();
    for t in [
        sdrkit_stack_dvoice::typed::IDENTIFIERS.def(),
        sdrkit_stack_dvoice::typed::ADJACENT.def(),
        sdrkit_stack_dvoice::typed::SEEN.def(),
    ] {
        let rows: Vec<Json> = table(r, t).into_iter().map(Json::Object).collect();
        tables.insert(t.key.into(), Json::Array(rows));
    }
    let detail = json!({
        "calls": calls,
        "messages": messages,
        "positions": positions,
        "talkgroups": talkgroups,
        "protocol": protocol,
        "offset_hz": field(r, "offset_hz"),
        "snr_db": field(r, "snr_db"),
        "deviation_hz": field(r, "deviation_hz"),
        "blocks_ok": field(r, "blocks_ok"),
        "blocks_bad": field(r, "blocks_bad"),
        "sequence": field(r, "reports"),
        "verdict": field(r, "verdict"),
    });
    let status = serde_json::to_value(r.status()).unwrap_or(Json::Null);
    json!({"detail": detail, "fields": fields, "status": status, "tables": tables})
}

/// The calls of a view, oldest first.
pub fn calls(r: &Json) -> Vec<Json> {
    let mut v: Vec<Json> = r["detail"]["calls"].as_array().cloned().unwrap_or_default();
    v.reverse();
    v
}

/// Reads a recorded vector. `None` (the reason printed) when the recording is missing.
pub fn vector(name: &str) -> Option<sdrkit_dev_test::sigmf::Recording> {
    sdrkit_dev_test::read_recording(&format!("dvoice/{name}"))
}

/// The expected values of the recordings.
pub fn expected() -> Json {
    sdrkit_dev_test::sigmf::read_expected("dvoice").expect("expected.json")
}

/// A short line per call, for messages.
pub fn show(calls: &[Json]) -> String {
    calls
        .iter()
        .map(|c| {
            format!(
                "slot {} {} {} -> {} start {:.2} end {:.2} frames {} {} {} end_seen {} late {} enc {}",
                c["slot"], c["kind"], c["src"], c["dst"], c["start_s"].as_f64().unwrap_or(0.0), c["end_s"].as_f64().unwrap_or(0.0),
                c["voice_frames"], c["alias"], c["text"], c["end_seen"], c["late_entry"], c["encryption"]["state"]
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The test signal of the stack at `rate` through the standard impairments of the framework (`gen_*`): a frequency offset, a
/// clock error and noise at `snr_db` in the bandwidth of the signal. One pass of the schedule.
pub fn impaired(id: &str, rate: f64, offset: f64, ppm: f64, snr_db: f64) -> Vec<Complex32> {
    impaired_with_seed(id, rate, offset, ppm, snr_db, 7)
}

/// As [`impaired`], with the noise of `seed`.
pub fn impaired_with_seed(
    id: &str,
    rate: f64,
    offset: f64,
    ppm: f64,
    snr_db: f64,
    seed: u64,
) -> Vec<Complex32> {
    let cfg = GenConfig::new(rate, values(id, &[])).with_seed(seed);
    let seconds = sdrkit_stack_dvoice::test_signal(id, &cfg)
        .expect("a digital voice stack")
        .seconds();
    sdrkit_dev_test::Harness::new(desc(id))
        .rate(rate)
        .seed(seed)
        .seconds(seconds)
        .set(&GEN_OFFSET_HZ, offset)
        .set(&GEN_CLOCK_PPM, ppm)
        .set(&GEN_NOISE, NoiseKind::Snr)
        .set(&GEN_SNR_DB, snr_db)
        .generate()
        .expect("the test signal")
}

/// The rows of a table that [`view`] put under `tables`.
pub fn table_rows(tables: &Json, key: &str) -> Vec<Json> {
    tables[key].as_array().cloned().unwrap_or_default()
}
