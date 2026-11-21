//! Helpers for the tests: make a signal with the standard impairments, feed it, apply settings, run commands, and read typed
//! fields and table rows.

use sdrkit_components::sim::{with_generator_settings, GenSetting};
use sdrkit_core::command::CommandError;
use sdrkit_core::publish::Declare;
use sdrkit_core::{
    owner, Complex32, Event, Generator, HostRequest, Output, Owner, Receiver, Record, Results,
    Schema, SettingError, SettingId, SettingSet, StackConfig, Value, ValueKind, Values,
};
use std::sync::Arc;

/// `v` in the kind that setting `id` of `schema` holds: a number becomes a whole number, a duration or a frequency, and a
/// text becomes the choice of that key. Other values stay as they are.
///
/// # Errors
///
/// [`SettingError::NotAChoice`] for a text that matches no choice of a choice setting.
pub fn typed(schema: &Schema, id: SettingId, v: Value) -> Result<Value, SettingError> {
    let Some(decl) = schema.setting(id) else {
        return Ok(v);
    };
    Ok(match (decl.field.kind, v) {
        (ValueKind::Choice, Value::Text(t)) => Value::Choice(decl.field.choice_index(&t).ok_or(
            SettingError::NotAChoice {
                id,
                index: u16::MAX,
            },
        )?),
        (ValueKind::I64, Value::F64(x)) if x.fract() == 0.0 => Value::I64(x as i64),
        (ValueKind::Duration, Value::F64(x)) => Value::Duration(x),
        (ValueKind::Hz, Value::F64(x)) => Value::Hz(x),
        (_, v) => v,
    })
}

/// Values of `schema` and the generator settings: the defaults, then `settings` (keys and values as [`typed`] takes them).
pub fn values(schema: &Schema, settings: &[(&str, Value)]) -> Values {
    let s = with_generator_settings(schema).expect("the settings join the generator settings");
    let mut v = Values::defaults(&s);
    for (k, x) in settings {
        let id = s.id(k).expect("a known key");
        let x = typed(&s, id, x.clone()).expect("a value of the setting's kind");
        v.set(id, &x).expect("an accepted value");
    }
    v
}

/// The stack configuration of `schema` with `settings`.
pub fn config(schema: Arc<Schema>, settings: &[(&str, Value)]) -> StackConfig {
    StackConfig::new(values(&schema, settings))
}

/// `seconds` of `gen` through the standard impairments of `settings` (`gen_*` keys), seeded with `seed`.
pub fn impaired(
    gen: Box<dyn Generator>,
    settings: &[(&str, Value)],
    seed: u64,
    seconds: f64,
) -> Vec<Complex32> {
    let s = GenSetting::schema();
    let mut v = Values::defaults(&s);
    v.set(GenSetting::Seed.id(), &Value::I64(seed as i64))
        .expect("a seed");
    for (k, x) in settings {
        let id = s.id(k).expect("a generator setting");
        let x = typed(&s, id, x.clone()).expect("a value of the setting's kind");
        v.set(id, &x).expect("an accepted value");
    }
    let rate = gen.rate_hz();
    let mut g = sdrkit_components::sim::impaired(gen, &v).expect("the impairments are accepted");
    let n = (seconds * rate).round() as usize;
    let mut x = vec![Complex32::default(); n];
    let got = g.generate(&mut x);
    x.truncate(got);
    x
}

/// Noise as SNR in the signal's bandwidth.
pub fn snr(db: f64) -> Vec<(&'static str, Value)> {
    vec![
        ("gen_noise", Value::Text("snr".into())),
        ("gen_snr_db", Value::F64(db)),
    ]
}

/// Applies setting `key` of `o`, validated against its schema. `v` is as [`typed`] accepts it.
///
/// # Errors
///
/// The refusal of the schema or of the owner.
pub fn set<O: Owner + ?Sized>(o: &mut O, key: &str, v: Value) -> Result<(), SettingError> {
    let id = o.schema().id(key)?;
    let v = typed(o.schema(), id, v)?;
    owner::apply(o, id, &v)
}

/// Runs command `key` of `o` with `args` (by argument key).
///
/// # Errors
///
/// The refusal of the command.
pub fn command<O: Owner + ?Sized>(
    o: &mut O,
    key: &str,
    args: &[(&str, Value)],
) -> Result<Option<Record>, CommandError> {
    let id = o.commands().id(key)?;
    let def = o.commands().def(id).expect("the command is declared");
    let mut r = def.new_args();
    for (k, v) in args {
        let fid = r.decl().id(k).expect("a declared argument");
        r.set_value(fid, v).expect("an argument of its kind");
    }
    owner::invoke(o, id, &r)
}

/// The output of a run besides the results.
#[derive(Default)]
pub struct Fed {
    /// Every event.
    pub events: Vec<Event>,
    /// Every host request.
    pub requests: Vec<HostRequest>,
}

/// Feeds `x` in pieces of `piece`, then ends the stream, so the last report holds everything.
pub fn feed(rx: &mut dyn Receiver, x: &[Complex32], piece: usize) -> Fed {
    let mut fed = Fed::default();
    let mut out = Output::default();
    for p in x.chunks(piece.max(1)) {
        out.clear();
        rx.feed(p, &mut out);
        fed.events.append(&mut out.events);
        fed.requests.append(&mut out.requests);
    }
    out.clear();
    rx.finish(&mut out);
    fed.events.append(&mut out.events);
    fed.requests.append(&mut out.requests);
    fed
}

/// The rows of table `key`, newest first.
pub fn rows<'a>(r: &'a Results, key: &str) -> Vec<&'a Record> {
    let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    t.rows_newest_first().map(|(_, row)| row).collect()
}

/// The row of table `key` whose text column `col` holds `want`.
pub fn row_with<'a>(
    r: &'a Results,
    key: &str,
    col: &impl Declare,
    want: &str,
) -> Option<&'a Record> {
    rows(r, key).into_iter().find(|c| text(c, col) == want)
}

/// The value of result field `key`.
pub fn value<'a>(r: &'a Results, key: &str) -> Option<&'a Value> {
    let id = r.decl().fields().id(key)?;
    r.fields().value(id).filter(|v| !matches!(v, Value::None))
}

/// A field's number (also for I64, Hz and Duration fields).
pub fn num(r: &Results, key: &str) -> Option<f64> {
    value(r, key).and_then(number)
}

/// The number a value holds.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::I64(x) => Some(*x as f64),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
        _ => None,
    }
}

/// The value of cell `f` of `row`, if it holds one.
pub fn cell<'a>(row: &'a Record, f: &impl Declare) -> Option<&'a Value> {
    row.decl()
        .id(f.key())
        .and_then(|id| row.value(id))
        .filter(|v| !matches!(v, Value::None))
}

/// A number cell of `row` (no value: NaN).
pub fn cnum(row: &Record, f: &impl Declare) -> f64 {
    cell(row, f).and_then(number).unwrap_or(f64::NAN)
}

/// A text cell of `row` ("" for no value).
pub fn text(row: &Record, f: &impl Declare) -> String {
    match cell(row, f) {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// The data folder of the repository.
fn data_folder() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data")
}

/// The rows of the HaLow region file of the repository.
pub fn halow_regions() -> Vec<sdrkit_presets::wlan::HalowRegion> {
    sdrkit_presets::wlan::read_halow_regions(
        &data_folder().join(sdrkit_presets::wlan::HALOW_REGIONS_FILE),
    )
    .expect("the HaLow region file")
}

/// The `halow` settings of a region of the data file.
pub fn halow_plan(code: &str) -> Vec<(String, Value)> {
    sdrkit_presets::wlan::halow_settings(&halow_regions(), code).expect("a region of the file")
}

/// The rows of the Wi-Fi domain file of the repository.
pub fn wifi_domains() -> Vec<sdrkit_presets::wlan::WifiDomain> {
    sdrkit_presets::wlan::read_wifi_domains(
        &data_folder().join(sdrkit_presets::wlan::WIFI_DOMAINS_FILE),
    )
    .expect("the Wi-Fi domain file")
}

/// The `wifi` settings of a domain of the data file.
pub fn wifi_plan(domain: &str, skip_dfs: bool) -> Vec<(String, Value)> {
    sdrkit_presets::wlan::wifi_settings(&wifi_domains(), domain, skip_dfs)
        .expect("a domain of the file")
}
