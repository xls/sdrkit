//! Helpers shared by the tests.
#![allow(dead_code)]

use num_complex::Complex32;
use sdrkit_core::publish::TableDef;
use sdrkit_core::{
    ChoiceDecl, Event, HostRequest, InputInfo, Output, Owner, Receiver, Results, StackConfig,
    StackDescriptor, Value, ValueKind, Values,
};
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeMap;

/// The settings of `desc` at their defaults, with `settings` given as text over them ("true" and "false" for yes and no, a
/// choice by its key).
pub fn values(desc: &StackDescriptor, settings: &[(&str, &str)]) -> Values {
    let schema = (desc.settings)();
    let mut v = Values::defaults(&schema);
    let mut j = Map::new();
    for (k, text) in settings {
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

/// A receiver of `desc` with these settings (as text, over the defaults).
pub fn make(desc: &StackDescriptor, settings: &[(&str, &str)]) -> Box<dyn Receiver> {
    (desc.make_receiver)(&StackConfig::new(values(desc, settings))).expect("the receiver builds")
}

/// Applies setting `key` of `rx` through the checked path.
pub fn set(rx: &mut dyn Receiver, key: &str, v: &Value) -> Result<(), sdrkit_core::SettingError> {
    let id = rx.schema().id(key)?;
    sdrkit_core::owner::apply(rx, id, v)
}

/// Runs command `key` of `rx` with its default arguments.
pub fn invoke(rx: &mut dyn Receiver, key: &str) {
    let id = rx.commands().id(key).expect("a command of the stack");
    let args = rx.commands().defs()[id.index()].new_args();
    sdrkit_core::owner::invoke(rx, id, &args).expect("the command runs");
}

/// What came out of a run.
#[derive(Default)]
pub struct Fed {
    /// The events.
    pub events: Vec<Event>,
    /// The host requests.
    pub requests: Vec<HostRequest>,
    /// The audio (the first channel of each frame).
    pub audio: Vec<f32>,
}

/// Feeds `x` in pieces of `chunk`, collecting the output.
pub fn feed(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Fed {
    let mut all = Fed::default();
    let mut out = Output::default();
    for c in x.chunks(chunk.max(1)) {
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        all.requests.append(&mut out.requests);
        for p in out.audio_runs() {
            all.audio
                .extend(p.samples.iter().step_by(usize::from(p.channels.max(1))));
        }
        out.clear();
    }
    all
}

/// Configures for samples at `rate`, centre `centre`, with the channel at `offset` from it (a recording).
pub fn configure(rx: &mut dyn Receiver, rate: f64, centre: f64, offset: f64) {
    rx.configure(&InputInfo::new(rate, centre, centre + offset).recording())
        .expect("configure");
}

/// The results of a receiver.
pub fn results(rx: &dyn Receiver) -> &Results {
    Owner::results(rx)
}

/// A typed value as text: a number, "yes" or "no", a choice by its key, a text as it is.
pub fn value_text(v: &Value, choices: &[ChoiceDecl]) -> String {
    match v {
        Value::Text(t) => t.clone(),
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => x.to_string(),
        Value::I64(x) => x.to_string(),
        Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
        Value::Choice(i) => choices
            .get(usize::from(*i))
            .map_or(String::new(), |c| c.key.to_string()),
        _ => String::new(),
    }
}

/// A field as a number (NaN when it is absent or not a number).
pub fn num(r: &Results, key: &str) -> f64 {
    let Some(id) = r.decl().field_id(key) else {
        return f64::NAN;
    };
    match r.fields().value(id) {
        Some(Value::I64(v)) => *v as f64,
        Some(Value::F64(v)) | Some(Value::Hz(v)) | Some(Value::Duration(v)) => *v,
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        _ => f64::NAN,
    }
}

/// A field as text ("" when it is absent or has no value).
pub fn text(r: &Results, key: &str) -> String {
    let Some(id) = r.decl().field_id(key) else {
        return String::new();
    };
    let choices = r
        .decl()
        .fields()
        .field(id)
        .map(|f| f.choices.to_vec())
        .unwrap_or_default();
    r.fields()
        .value(id)
        .map(|v| value_text(v, &choices))
        .unwrap_or_default()
}

/// The rows of a declared table, oldest first, each as its cells' text by column key.
pub fn rows(r: &Results, def: &TableDef) -> Vec<BTreeMap<String, String>> {
    let Some(t) = r.decl().table_id(def.key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    let mut v: Vec<BTreeMap<String, String>> = t
        .rows_newest_first()
        .map(|(_, rec)| {
            rec.iter()
                .map(|(d, x)| (d.key.to_string(), value_text(x, &d.choices)))
                .collect()
        })
        .collect();
    v.reverse();
    v
}

/// Makes sure that every event is a kind that `kinds` lists and that no event with a known frequency carries 0 Hz.
pub fn check_events(kinds: &[&str], events: &[Event], freq_known: bool) {
    for e in events {
        assert!(kinds.contains(&e.kind()), "an undeclared event: {e:?}");
        if freq_known {
            assert!(e.freq_hz > 0.0, "an event with no frequency: {e:?}");
        }
        assert!(e.time.is_finite() && e.time >= 0.0, "{e:?}");
    }
}

/// The event kinds that a declaration lists.
pub fn kinds(decl: &sdrkit_core::ResultsDecl) -> Vec<&str> {
    decl.events().iter().map(|e| e.kind.as_ref()).collect()
}

/// An event field as text ("" when it is absent).
pub fn field(e: &Event, key: &str) -> String {
    e.fields
        .iter()
        .find(|(d, _)| d.key == key)
        .map(|(d, v)| value_text(v, &d.choices))
        .unwrap_or_default()
}

/// Real audio as IQ (Q = 0).
pub fn audio_iq(a: &[f32]) -> Vec<Complex32> {
    a.iter().map(|&v| Complex32::new(v, 0.0)).collect()
}

/// Complex Gaussian noise of `sigma` for each component, added to `x` (from `sdrkit-algo-sim`).
pub fn add_noise(x: &mut [Complex32], sigma: f64, seed: u64) {
    let mut n =
        sdrkit_algo_sim::noise::Awgn::with_power(2.0 * sigma * sigma, seed).expect("a noise power");
    n.process(x);
}

/// Complex Gaussian noise added to `x` (at `rate`) so that the mean power of its samples that are not zero stands `snr_db` above
/// the noise in `bandwidth_hz`.
pub fn add_noise_snr(x: &mut [Complex32], rate: f64, snr_db: f64, bandwidth_hz: f64, seed: u64) {
    let (sum, n) = x
        .iter()
        .map(|z| z.norm_sqr() as f64)
        .filter(|&p| p > 0.0)
        .fold((0.0, 0usize), |(s, n), p| (s + p, n + 1));
    let p = sum / n.max(1) as f64;
    let per_hz = p / 10f64.powf(snr_db / 10.0) / bandwidth_hz;
    add_noise(x, (per_hz * rate / 2.0).sqrt(), seed);
}

/// Real Gaussian noise of `sigma`, added to `x` (from `sdrkit-algo-sim`).
pub fn add_real_noise(x: &mut [f32], sigma: f64, seed: u64) {
    let mut n =
        sdrkit_algo_sim::noise::Awgn::with_power(sigma * sigma, seed).expect("a noise power");
    n.process_real(x);
}
