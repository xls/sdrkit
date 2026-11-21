//! Helpers that the tests of the crate share: receivers and generators with settings, typed reads of results and tables, the
//! recordings.
#![allow(dead_code)]

use sdrkit_core::publish::{field, Field, TableDef};
pub use sdrkit_core::Owner;
use sdrkit_core::{
    owner, Complex32, GenConfig, Generator, InputInfo, Output, Receiver, Record, Results,
    SettingError, StackConfig, StackDescriptor, Value, Values,
};
use sdrkit_dev_test::sigmf;
pub use serde_json::{json, Value as Json};

/// The settings of `desc` with the generator settings, at their defaults, then `set` (key and value in the settings JSON form).
pub fn values(desc: &StackDescriptor, set: &[(&str, Json)]) -> Values {
    let schema = sdrkit_components::sim::with_generator_settings(&(desc.settings)())
        .expect("the settings take the generator settings");
    let mut v = Values::defaults(&schema);
    for (k, x) in set {
        put(&mut v, k, x.clone());
    }
    v
}

/// The generator settings alone, at their defaults.
pub fn gen_values() -> Values {
    use sdrkit_core::SettingSet;
    Values::defaults(&sdrkit_components::sim::GenSetting::schema())
}

/// Sets `key` of `v` to `x` in the settings JSON form. A whole number given as a float is taken for a count.
pub fn put(v: &mut Values, key: &str, x: Json) {
    let schema = v.schema().clone();
    let kind = schema
        .id(key)
        .ok()
        .and_then(|id| schema.setting(id))
        .map(|s| s.field.kind);
    let x = match (&x, kind) {
        (Json::Number(n), Some(sdrkit_core::ValueKind::I64)) => {
            json!(n.as_f64().unwrap_or(0.0) as i64)
        }
        _ => x,
    };
    v.update_from_json(&json!({ key: x }))
        .unwrap_or_else(|e| panic!("{key}: {e:?}"));
}

/// A receiver of `desc` with its default settings and then `set`. A refused setting fails the test.
pub fn receiver(desc: &StackDescriptor, set: &[(&str, Json)]) -> Box<dyn Receiver> {
    let mut rx = (desc.make_receiver)(&StackConfig::new(values(desc, &[]))).unwrap();
    for (k, v) in set {
        self::set(rx.as_mut(), k, v.clone()).unwrap_or_else(|e| panic!("{k}: {e:?}"));
    }
    rx
}

/// Applies setting `key` of `rx` with `value` in the settings JSON form, through the checked path.
///
/// # Errors
///
/// The refusal of the schema or of the receiver.
pub fn set(rx: &mut dyn Receiver, key: &str, value: Json) -> Result<(), SettingError> {
    let schema = rx.schema().clone();
    let id = schema.id(key)?;
    let mut v = Values::defaults(&schema);
    v.update_from_json(&json!({ key: value }))
        .map_err(|e| match e {
            sdrkit_core::ValuesError::Setting(s) => s,
            _ => SettingError::Malformed { id, at: None },
        })?;
    let value = v.value(id).expect("the setting has a value").clone();
    owner::apply(rx, id, &value)
}

/// Runs command `key` of `rx` with the row argument `row`, if given.
pub fn invoke(rx: &mut dyn Receiver, key: &str, row: Option<&str>) {
    let id = rx.commands().id(key).expect("the stack has the command");
    let mut args = rx.commands().defs()[id.index()].new_args();
    if let Some(r) = row {
        let row: Field<field::Text> = Field::new("row", "Row", "");
        args.set(&row, r);
    }
    owner::invoke(rx, id, &args).expect("the command runs");
}

/// The generator of `desc` at `rate_hz` with its default settings and seed `seed`.
pub fn generator(desc: &StackDescriptor, rate_hz: f64, seed: u64) -> Box<dyn Generator> {
    generator_with(desc, rate_hz, seed, &[])
}

/// The generator of `desc` at `rate_hz` with seed `seed` and the settings `set`.
pub fn generator_with(
    desc: &StackDescriptor,
    rate_hz: f64,
    seed: u64,
    set: &[(&str, Json)],
) -> Box<dyn Generator> {
    (desc.make_generator.expect("every stack has a generator"))(
        &GenConfig::new(rate_hz, values(desc, set)).with_seed(seed),
    )
    .unwrap()
}

/// Feeds `x` in pieces of `piece`.
pub fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Output {
    let mut all = Output::default();
    let mut out = Output::default();
    for c in x.chunks(piece.max(1)) {
        rx.feed(c, &mut out);
        all.events.append(&mut out.events);
        for p in out.audio_runs() {
            all.push_audio(p.samples, p.rate_hz, p.channels);
        }
        all.media.append(&mut out.media);
        out.clear();
    }
    all
}

/// Runs a generator to its end (or `max_s` seconds).
pub fn generate(g: &mut dyn Generator, max_s: f64) -> Vec<Complex32> {
    let n = (g.rate_hz() * max_s) as usize;
    let mut x = vec![Complex32::new(0.0, 0.0); n];
    let k = g.generate(&mut x);
    x.truncate(k);
    x
}

/// Reads a recording of this crate. A missing recording fails the test (set `SDRKIT_RECORDINGS`).
pub fn vector(name: &str) -> sigmf::Recording {
    sdrkit_dev_test::recordings::read_recording(&format!("hfdata/{name}"))
        .unwrap_or_else(|| panic!("the recording hfdata/{name} is missing"))
}

/// The expected results of a recording.
pub fn expected(name: &str) -> Json {
    sigmf::read_expected("hfdata").expect("expected.json")[format!("{name}.sigmf-meta")].clone()
}

/// Signals the end of the input to the receiver and returns its final results.
pub fn end(rx: &mut dyn Receiver) -> Output {
    let mut out = Output::default();
    rx.finish(&mut out);
    out
}

/// Feeds a recording in pieces of 0.1 s with the dial at its centre, then notifies the receiver that the input ended.
pub fn run_vector(rx: &mut dyn Receiver, rec: &sigmf::Recording) -> Output {
    rx.configure(&InputInfo::new(rec.rate_hz, rec.centre_hz, rec.centre_hz).recording())
        .expect("configure");
    let mut out = feed(rx, &rec.samples, (rec.rate_hz / 10.0) as usize);
    let mut last = end(rx);
    out.events.append(&mut last.events);
    out
}

/// A row of a table: each column key with its cell. A choice is its label as text.
pub type Row = Vec<(&'static str, Value)>;

/// A cell as the tests read it: a choice becomes its label as text.
fn readable(r: &Record, key: &str) -> Value {
    let Some(id) = r.decl().id(key) else {
        return Value::None;
    };
    match r.value(id) {
        Some(Value::Choice(_)) => {
            Value::Text(r.text(id).map(|t| t.to_string()).unwrap_or_default())
        }
        Some(v) => v.clone(),
        None => Value::None,
    }
}

/// The rows of the declared table `def`, oldest first.
pub fn rows(r: &Results, def: &TableDef) -> Vec<Row> {
    let Some(table) = r.decl().table_id(def.key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    let mut v: Vec<Row> = table
        .rows_newest_first()
        .map(|(_, rec)| {
            def.columns
                .iter()
                .map(|c| (c.key(), readable(rec, c.key())))
                .collect()
        })
        .collect();
    v.reverse();
    v
}

/// The cell `key` of a row.
pub fn cell(row: &[(&'static str, Value)], key: &str) -> Value {
    row.iter()
        .find(|(k, _)| *k == key)
        .map_or(Value::None, |(_, v)| v.clone())
}

/// The text of a value, empty when it is not text.
pub fn as_text(v: &Value) -> String {
    match v {
        Value::Text(s) => s.clone(),
        _ => String::new(),
    }
}

/// The number of a value, `NaN` when it is not a number.
pub fn as_num(v: &Value) -> f64 {
    match v {
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => *x,
        Value::I64(i) => *i as f64,
        _ => f64::NAN,
    }
}

/// The text of cell `key`.
pub fn text(row: &[(&'static str, Value)], key: &str) -> String {
    as_text(&cell(row, key))
}

/// The number of cell `key`.
pub fn num(row: &[(&'static str, Value)], key: &str) -> f64 {
    as_num(&cell(row, key))
}

/// The results field `key` (a choice as its label), `Value::None` when it has none.
pub fn value<R: Receiver + ?Sized>(rx: &R, key: &str) -> Value {
    readable(rx.results().fields(), key)
}

/// The number of the results field `key`.
pub fn field<R: Receiver + ?Sized>(rx: &R, key: &str) -> Option<f64> {
    Some(as_num(&value(rx, key))).filter(|x| !x.is_nan())
}

/// The text of the results field `key` (a choice as its label), empty when it has none.
pub fn field_text<R: Receiver + ?Sized>(rx: &R, key: &str) -> String {
    as_text(&value(rx, key))
}

/// A saved state with its second half cut off.
pub fn half(state: &sdrkit_core::StateBlob) -> sdrkit_core::StateBlob {
    let b = state.bytes();
    sdrkit_core::StateBlob::new(
        state.owner().to_owned(),
        state.version(),
        b[..b.len() / 2].to_vec(),
    )
}

/// Whether `d` declares the kind of event `e`.
pub fn declared(d: &sdrkit_core::ResultsDecl, e: &sdrkit_core::Event) -> bool {
    d.events().iter().any(|k| k.kind == e.kind())
}

/// A channel for the tests of the serial-tone and naval HF stacks: frequency offset, clock error, fading and noise.
#[derive(Clone, Debug, Default)]
pub struct Channel {
    /// SNR in `bw_hz`, dB; `None` for no noise.
    pub snr_db: Option<f64>,
    /// The band the SNR is measured in, Hz (3000 when 0).
    pub bw_hz: f64,
    /// Frequency offset, Hz.
    pub offset_hz: f64,
    /// Frequency drift, Hz a second.
    pub drift_hz_s: f64,
    /// Sample clock error, ppm.
    pub ppm: f64,
    /// A fading profile.
    pub fading: Option<sdrkit_algo_sim::fading::Profile>,
    /// The seed of the noise and the fading.
    pub seed: u64,
}

/// `x` (at `rate`, unit power while the signal is on) through `ch`.
pub fn through(x: &[Complex32], rate: f64, ch: &Channel) -> Vec<Complex32> {
    use sdrkit_algo_sim::rng::Rng;
    let mut y: Vec<Complex32> = if ch.ppm != 0.0 {
        // Linear interpolation at the receiver's clock.
        let r = 1.0 + ch.ppm * 1e-6;
        let n = ((x.len() as f64 - 1.0) / r) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 * r;
                let k = t as usize;
                let f = (t - k as f64) as f32;
                x[k] * (1.0 - f) + x[(k + 1).min(x.len() - 1)] * f
            })
            .collect()
    } else {
        x.to_vec()
    };
    if let Some(p) = &ch.fading {
        let mut tdl = sdrkit_algo_sim::fading::Tdl::from_profile(p, rate, ch.seed ^ 0x5A5A)
            .expect("a valid profile");
        tdl.process(&mut y);
    }
    if ch.offset_hz != 0.0 || ch.drift_hz_s != 0.0 {
        let mut ph = 0.0f64;
        for (i, v) in y.iter_mut().enumerate() {
            let t = i as f64 / rate;
            let f = ch.offset_hz + ch.drift_hz_s * t;
            ph += std::f64::consts::TAU * f / rate;
            *v *= Complex32::new(ph.cos() as f32, ph.sin() as f32);
        }
    }
    if let Some(snr) = ch.snr_db {
        let bw = if ch.bw_hz > 0.0 { ch.bw_hz } else { 3000.0 };
        let p = 10f64.powf(-snr / 10.0) * rate / bw;
        let mut rng = Rng::new(ch.seed.wrapping_add(17));
        for v in y.iter_mut() {
            *v += rng.complex_normal(p);
        }
    }
    y
}

/// `secs` of the test signal of `desc` at `rate` with the settings `set`.
pub fn test_signal(
    desc: &StackDescriptor,
    rate: f64,
    set: &[(&str, Json)],
    secs: f64,
) -> Vec<Complex32> {
    let mut g = generator_with(desc, rate, 1, set);
    generate(g.as_mut(), secs)
}

/// The texts of the events of kind `kind` among `out`.
pub fn event_texts(out: &Output, kind: &str) -> Vec<String> {
    out.events
        .iter()
        .filter(|e| e.kind() == kind)
        .filter_map(|e| e.get(&sdrkit_core::publish::keys::TEXT).map(str::to_owned))
        .collect()
}

/// Runs `x` (24 kHz, the dial at 0 Hz) through a fresh receiver of `desc` in pieces of `piece`, and returns its output with
/// the output of the end of the input.
pub fn run_24k(
    desc: &StackDescriptor,
    x: &[Complex32],
    piece: usize,
) -> (Box<dyn Receiver>, Output) {
    let mut rx = receiver(desc, &[]);
    let f = desc.tuning.default_hz;
    rx.configure(&InputInfo::new(24e3, f, f).recording())
        .unwrap();
    let mut out = feed(rx.as_mut(), x, piece);
    let mut e = end(rx.as_mut());
    out.events.append(&mut e.events);
    (rx, out)
}

/// The rows of a data file of the repository's `data/hfdata/` folder (comment lines and the header left out).
pub fn data_rows(name: &str) -> Vec<Vec<String>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../data/hfdata")
        .join(name);
    let text = std::fs::read_to_string(&path).expect("the data file");
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .skip(1)
        .map(|l| l.split(',').map(str::to_owned).collect())
        .collect()
}
