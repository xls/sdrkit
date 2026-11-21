#![allow(dead_code)]
//! Helpers shared by the marine tests.

use sdrkit_components::sim::GenSetting;
use sdrkit_core::publish::{field, Field};
use sdrkit_core::{
    owner, Complex32, Event, Generator, HostRequest, InputInfo, Output, Receiver, Record,
    ResultsDecl, SettingSet, StackConfig, StackDescriptor, Value, Values,
};
use serde_json::{json, Map};

/// A JSON value: the settings form, and the expected values of a recording.
pub type Json = serde_json::Value;

/// Values of `schema` from its defaults with the settings JSON form of `set` on top.
fn values_of(schema: &std::sync::Arc<sdrkit_core::Schema>, set: &[(&str, Json)]) -> Values {
    let mut v = Values::defaults(schema);
    let obj: Map<String, Json> = set
        .iter()
        .map(|(k, x)| ((*k).to_owned(), x.clone()))
        .collect();
    v.update_from_json(&Json::Object(obj))
        .expect("the settings are valid");
    v
}

/// A receiver of `d` with its default settings overlaid with `set` (the settings JSON form: numbers, booleans, texts, choice
/// keys).
pub fn receiver(
    d: &StackDescriptor,
    submode: Option<&str>,
    set: &[(&str, Json)],
) -> Box<dyn Receiver> {
    let v = values_of(&(d.settings)(), set);
    (d.make_receiver)(&StackConfig::new(v).with_submode_opt(submode.map(|s| s.to_string())))
        .expect("the receiver builds")
}

/// Applies the setting `key` of a receiver through the checked path.
pub fn apply(rx: &mut dyn Receiver, key: &str, v: Value) {
    let id = rx.schema().id(key).expect("the stack has the setting");
    owner::apply(rx, id, &v).expect("the setting is taken");
}

/// Runs the command `key` of a receiver through the checked path, with the `row` argument when one is given.
pub fn invoke(rx: &mut dyn Receiver, key: &str, row: Option<&str>) {
    let id = rx.commands().id(key).expect("the stack has the command");
    let mut args = rx.commands().defs()[id.index()].new_args();
    if let Some(r) = row {
        let row: Field<field::Text> = Field::new("row", "Row", "");
        args.set(&row, r);
    }
    owner::invoke(rx, id, &args).expect("the command runs");
}

/// Feeds `x` in pieces of `chunk`, then ends the stream, collecting events.
pub fn run(rx: &mut dyn Receiver, x: &[Complex32], chunk: usize) -> Vec<Event> {
    let mut out = Output::default();
    let mut ev = Vec::new();
    for c in x.chunks(chunk.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
    }
    out.clear();
    rx.finish(&mut out);
    ev.append(&mut out.events);
    ev
}

/// Configures for a recording at `rate`, centre `centre`, tuned `tuned`.
pub fn configure(rx: &mut dyn Receiver, rate: f64, centre: f64, tuned: f64) {
    rx.configure(&InputInfo::new(rate, centre, tuned).recording())
        .expect("configures");
}

/// Reads a marine vector; `None` (the reason printed) when the recording is missing.
pub fn vector(name: &str) -> Option<sdrkit_dev_test::sigmf::Recording> {
    sdrkit_dev_test::read_recording(&format!("marine/{name}"))
}

/// The marine expected.json entry of `file`.
pub fn expected(file: &str) -> Json {
    let e = sdrkit_dev_test::sigmf::read_expected("marine").expect("expected.json");
    e["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["file"] == file)
        .cloned()
        .expect("entry")
}

/// Silence: signal time with nothing in it.
pub fn silence(n: usize) -> Vec<Complex32> {
    vec![Complex32::default(); n]
}

/// The bits of the AIS message the `!AIVDM` sentences carry (parts in order). `None` for a bad sentence or an incomplete message.
pub fn bits_of(lines: &[&str]) -> Option<Vec<u8>> {
    use sdrkit_util_geo::nmea::{self, aivdm};
    let mut a = aivdm::Assembler::new();
    for l in lines {
        let s = nmea::parse(l).ok()?;
        let v = aivdm::Vdm::from_sentence(&s).ok()?;
        if let Some(m) = a.push(&v).ok()? {
            return Some(m.bits.to_vec());
        }
    }
    None
}

/// The `!AIVDM` sentences of AIS message `bits` on `channel`.
pub fn sentences(bits: &[u8], channel: char) -> Vec<String> {
    let mut out = Vec::new();
    sdrkit_stack_marine::ais::message::sentences(bits, channel, &mut 0, &mut out)
        .expect("sentences");
    out
}

/// Feeds `x` in pieces of `chunk`, collecting events and host requests.
pub fn run_requests(
    rx: &mut dyn Receiver,
    x: &[Complex32],
    chunk: usize,
) -> (Vec<Event>, Vec<HostRequest>) {
    let mut out = Output::default();
    let (mut ev, mut req) = (Vec::new(), Vec::new());
    for c in x.chunks(chunk.max(1)) {
        out.clear();
        rx.feed(c, &mut out);
        ev.append(&mut out.events);
        req.append(&mut out.requests);
    }
    (ev, req)
}

/// The typed result field `key`.
pub fn field<'a>(r: &'a sdrkit_core::Results, key: &str) -> Option<&'a Value> {
    let id = r.decl().fields().id(key)?;
    r.fields().value(id).filter(|v| !matches!(v, Value::None))
}

/// A number of `v`: a whole number, a real number, a frequency, a duration or a time.
pub fn num_of(v: &Value) -> Option<f64> {
    match v {
        Value::I64(x) => Some(*x as f64),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
        _ => None,
    }
}

/// The number of result field `key`.
pub fn num(r: &sdrkit_core::Results, key: &str) -> Option<f64> {
    field(r, key).and_then(num_of)
}

/// The text of result field `key`: a text, or the key of a choice.
pub fn text(r: &sdrkit_core::Results, key: &str) -> Option<String> {
    let id = r.decl().fields().id(key)?;
    match r.fields().value(id)? {
        Value::Text(t) => Some(t.clone()),
        Value::Choice(i) => {
            let d = r.decl().fields().field(id)?;
            d.choices.get(usize::from(*i)).map(|c| c.key.to_string())
        }
        _ => None,
    }
}

/// The yes or no of result field `key`.
pub fn flag(r: &sdrkit_core::Results, key: &str) -> Option<bool> {
    match field(r, key)? {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

/// A whole-number typed field of the results (0 when missing).
pub fn count(r: &sdrkit_core::Results, key: &str) -> u64 {
    num(r, key).map_or(0, |x| x as u64)
}

/// The impairments of an AIS test signal: noise at `snr_db` in 48 kHz against a level-1 burst, a
/// carrier offset, a clock error, and DC and 8-bit samples with the signal scaled so its peaks (`max_level` bursts at once) stay
/// below 0.9 of full scale.
#[derive(Clone, Copy)]
pub struct Impair {
    pub snr_db: Option<f64>,
    pub offset_hz: f64,
    pub clock_ppm: f64,
    pub dc: f64,
    pub quantize8: bool,
    pub max_level: f64,
}

impl Default for Impair {
    fn default() -> Self {
        Impair {
            snr_db: Some(30.0),
            offset_hz: 0.0,
            clock_ppm: 0.0,
            dc: 0.0,
            quantize8: false,
            max_level: 1.0,
        }
    }
}

impl Impair {
    /// Noise at `snr_db` only.
    pub fn snr(snr_db: f64) -> Self {
        Impair {
            snr_db: Some(snr_db),
            ..Default::default()
        }
    }

    /// The standard `gen_*` settings for samples at `rate`.
    pub fn values(&self, rate: f64) -> Values {
        let mut set: Vec<(&str, Json)> = Vec::new();
        let mut put = |k: &'static str, x: Json| set.push((k, x));
        // A level-1 burst at amplitude 0.3, all scaled so the peaks stay below 0.9.
        let sigma = self.snr_db.map_or(0.0, |s| {
            let n48 = 0.09 / 10f64.powf(s / 10.0);
            (n48 * rate / 48e3 / 2.0).sqrt()
        });
        let peak =
            0.3 * self.max_level * 2.0 + 4.0 * sigma * std::f64::consts::SQRT_2 + self.dc.abs();
        let scale = (0.9 / peak).min(1.0);
        put("gen_level_dbfs", json!(20.0 * (0.3 * scale).log10()));
        if let Some(s) = self.snr_db {
            put("gen_noise", json!("snr"));
            put("gen_snr_db", json!(s));
        }
        put("gen_offset_hz", json!(self.offset_hz));
        put("gen_clock_ppm", json!(self.clock_ppm));
        put("gen_dc_i", json!(self.dc * scale));
        put("gen_dc_q", json!(0.5 * self.dc * scale));
        if self.quantize8 {
            put("gen_adc_bits", json!(8));
        }
        values_of(&GenSetting::schema(), &set)
    }

    /// `g` through these impairments.
    pub fn apply(&self, g: Box<dyn Generator>) -> Box<dyn Generator> {
        let v = self.values(g.rate_hz());
        sdrkit_components::sim::impaired(g, &v).expect("the impairments are valid")
    }
}

/// `secs` of the generator's samples.
pub fn generate(g: &mut dyn Generator, secs: f64) -> Vec<Complex32> {
    let mut x = vec![Complex32::default(); (secs * g.rate_hz()) as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

/// The AIS bursts rendered for `r.duration_s` through the impairments `imp`.
pub fn ais_signal(
    bursts: &[sdrkit_stack_marine::ais::generator::BurstSpec],
    r: &sdrkit_stack_marine::ais::generator::Render,
    imp: &Impair,
) -> Vec<Complex32> {
    use sdrkit_stack_marine::ais::generator::{gmsk_burst, Streamer};
    let mut imp_r = Impair { ..*imp };
    imp_r.max_level = bursts.iter().map(|b| b.level).fold(1.0, f64::max);
    let g = Streamer::new(bursts.iter().map(|b| gmsk_burst(b, r)).collect(), r.clone());
    let mut g = imp_r.apply(Box::new(g));
    generate(g.as_mut(), r.duration_s)
}

/// Samples made before, played as a generator of the AIS test signal (for impairments added after).
pub struct Replay {
    x: Vec<Complex32>,
    pos: usize,
    rate: f64,
}

impl Replay {
    /// `x` at `rate`.
    pub fn new(x: Vec<Complex32>, rate: f64) -> Self {
        Replay { x, pos: 0, rate }
    }
}

impl Generator for Replay {
    fn rate_hz(&self) -> f64 {
        self.rate
    }
    fn generate(&mut self, out: &mut [Complex32]) -> usize {
        let n = out.len().min(self.x.len() - self.pos);
        out[..n].copy_from_slice(&self.x[self.pos..self.pos + n]);
        self.pos += n;
        n
    }
    fn signal_info(&self) -> Option<sdrkit_core::SignalInfo> {
        Some(sdrkit_stack_marine::ais::generator::signal_info())
    }
}

/// `secs` of a marine HF programme at `rate` (played once), through the engine's impairments set by the `gen_*` values `imp`.
pub fn hf_signal(
    prog: Vec<sdrkit_stack_marine::hfgen::Segment>,
    info: sdrkit_core::SignalInfo,
    rate: f64,
    secs: f64,
    imp: &[(&str, Json)],
) -> Vec<Complex32> {
    use sdrkit_stack_marine::hfgen::{HfGenerator, HfOptions};
    let g = HfGenerator::new(
        prog,
        HfOptions {
            rate,
            once: true,
            info,
        },
    )
    .expect("the programme fits the rate");
    let mut g = impaired(Box::new(g), imp);
    generate(g.as_mut(), secs)
}

/// Noise at `snr_db` in the signal's own bandwidth, as `gen_*` values.
pub fn snr(snr_db: f64) -> [(&'static str, Json); 2] {
    [("gen_noise", json!("snr")), ("gen_snr_db", json!(snr_db))]
}

/// The rows of table `name`, oldest first (empty when the table is missing).
pub fn rows(r: &sdrkit_core::Results, name: &str) -> Vec<Record> {
    let Some(t) = r.decl().table_id(name).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    let mut v: Vec<Record> = t.rows_newest_first().map(|(_, row)| row.clone()).collect();
    v.reverse();
    v
}

/// The typed cells of table `name` in column order, oldest row first (empty when the table is missing).
pub fn typed_rows(r: &sdrkit_core::Results, name: &str) -> Vec<Vec<Value>> {
    rows(r, name)
        .iter()
        .map(|row| row.iter().map(|(_, v)| v.clone()).collect())
        .collect()
}

/// Makes sure that every event is a declared kind and carries a frequency.
pub fn check_events(decl: &ResultsDecl, ev: &[Event]) {
    for e in ev {
        assert!(
            decl.events().iter().any(|d| d.kind == e.kind()),
            "{} is not declared",
            e.kind()
        );
        assert!(e.freq_hz != 0.0, "{} is sent at 0 Hz", e.kind());
    }
}

/// `g` through the engine's impairments set by the `gen_*` values `imp`.
pub fn impaired(g: Box<dyn Generator>, imp: &[(&str, Json)]) -> Box<dyn Generator> {
    let v = values_of(&GenSetting::schema(), imp);
    sdrkit_components::sim::impaired(g, &v).expect("the impairments are valid")
}

/// The text of each cell of table `name`, newest row first (empty when the table is missing).
pub fn text_rows(r: &sdrkit_core::Results, name: &str) -> Vec<Vec<String>> {
    let Some(t) = r.decl().table_id(name).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first()
        .map(|(_, row)| {
            (0..row.decl().len())
                .map(|i| {
                    row.text(sdrkit_core::FieldId(i as u16))
                        .map(|t| t.to_string())
                        .unwrap_or_default()
                })
                .collect()
        })
        .collect()
}

/// Field `key` of an event in the JSON form of the expected values: a text, the key of a choice, a number, a yes or no, a
/// position as `{"lat", "lon"}`. Null when the event lacks it.
pub fn field_json(e: &Event, key: &str) -> Json {
    let Some(id) = e.fields.decl().id(key) else {
        return Json::Null;
    };
    match e.fields.value(id) {
        Some(Value::Text(t)) => json!(t),
        Some(Value::Choice(i)) => {
            let d = e.fields.decl().field(id).expect("a declared field");
            json!(d.choices[usize::from(*i)].key)
        }
        Some(Value::Bool(b)) => json!(b),
        Some(Value::I64(x)) => json!(x),
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => json!(x),
        Some(Value::Position { lat, lon }) => json!({ "lat": lat, "lon": lon }),
        _ => Json::Null,
    }
}

/// The parts of a stack's saved state: each component's id and its own state.
fn parts(
    state: &sdrkit_core::StateBlob,
) -> (
    sdrkit_core::StateBlob,
    Vec<(String, sdrkit_core::StateBlob)>,
) {
    let graph: sdrkit_core::StateBlob =
        serde_json::from_slice(state.bytes()).expect("a graph state");
    let parts = serde_json::from_slice(graph.bytes()).expect("the parts of a graph state");
    (graph, parts)
}

/// Whether a stack's saved state holds a part of format `owner`.
pub fn holds(state: &sdrkit_core::StateBlob, owner: &str) -> bool {
    parts(state).1.iter().any(|(_, p)| p.owner() == owner)
}

/// A stack's saved state with the part of format `from` marked as format `to`.
pub fn renamed(
    state: &sdrkit_core::StateBlob,
    from: &str,
    to: &'static str,
) -> sdrkit_core::StateBlob {
    use sdrkit_core::StateBlob;
    let (graph, list) = parts(state);
    let list: Vec<(String, StateBlob)> = list
        .into_iter()
        .map(|(id, p)| {
            if p.owner() == from {
                (id, StateBlob::new(to, p.version(), p.bytes().to_vec()))
            } else {
                (id, p)
            }
        })
        .collect();
    let graph = StateBlob::new(
        graph.owner().to_owned(),
        graph.version(),
        serde_json::to_vec(&list).expect("the parts serialise"),
    );
    StateBlob::new(
        state.owner().to_owned(),
        state.version(),
        serde_json::to_vec(&graph).expect("a graph state serialises"),
    )
}

/// A blob that is no stack's state.
pub fn not_a_state() -> sdrkit_core::StateBlob {
    sdrkit_core::StateBlob::new("not a state", 1, Vec::new())
}

/// Applies the setting `key` of a receiver through the checked path, and returns the result.
pub fn try_apply(
    rx: &mut dyn Receiver,
    key: &str,
    v: Value,
) -> Result<(), sdrkit_core::SettingError> {
    let id = rx.schema().id(key)?;
    owner::apply(rx, id, &v)
}

/// The index of choice `key` of setting `setting` of a receiver.
pub fn choice(rx: &dyn Receiver, setting: &str, key: &str) -> u16 {
    let s = rx.schema();
    let id = s.id(setting).expect("the stack has the setting");
    s.setting(id)
        .unwrap()
        .field
        .choice_index(key)
        .expect("a choice of the setting")
}

/// The marine VHF channel plan `id` of the repository's data folder.
pub fn plan_of(id: &str) -> sdrkit_presets::marine::MarinePlan {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data")
        .join(sdrkit_presets::marine::VHF_CHANNELS);
    let plans = sdrkit_presets::marine::read_plans(&path).expect("the plans load");
    plans
        .into_iter()
        .find(|p| p.id == id)
        .expect("the plan is in the file")
}
