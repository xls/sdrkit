//! Helpers that the tests share: the impairments of the framework on a clean generator, and reads of the typed results by key.

use sdrkit_components::sim::with_generator_settings;
use sdrkit_core::publish::TableDef;
use sdrkit_core::{
    owner, Complex32, Generator, Receiver, Record, Results, Schema, StackConfig, Value, Values,
};
use serde_json::Value as Json;
use std::sync::Arc;

/// A setting in the settings JSON form: its key and its value.
pub type Set<'a> = (&'a str, Json);

/// The values of `schema` with `settings` (the settings JSON form) over the defaults.
///
/// # Panics
///
/// If `schema` refuses a key or a value.
pub fn values(schema: &Arc<Schema>, settings: &[Set]) -> Values {
    let mut v = Values::defaults(schema);
    let map: serde_json::Map<String, Json> = settings
        .iter()
        .map(|(k, x)| ((*k).to_string(), x.clone()))
        .collect();
    v.update_from_json(&Json::Object(map))
        .expect("the schema takes these settings");
    v
}

/// The stack configuration of the stack settings `schema` with `settings`.
pub fn stack_config(schema: Arc<Schema>, settings: &[Set]) -> StackConfig {
    StackConfig::new(values(&schema, settings))
}

/// Applies the setting `key` with `value` (the settings JSON form) to `rx` through the checked path.
///
/// # Errors
///
/// If the schema of `rx` has no such setting or refuses the value, or `rx` refuses it.
pub fn apply(
    rx: &mut dyn Receiver,
    key: &str,
    value: Json,
) -> Result<(), Box<dyn std::error::Error>> {
    let id = rx.schema().id(key)?;
    let mut v = Values::defaults(rx.schema());
    v.update_from_json(&serde_json::json!({ key: value }))?;
    let typed = v.value(id).cloned().unwrap_or(Value::None);
    owner::apply(rx, id, &typed)?;
    Ok(())
}

/// Runs the command `key` of `rx` with the arguments `args` (the settings JSON form).
///
/// # Errors
///
/// If `rx` has no such command, refuses an argument, or the command fails.
pub fn invoke(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[Set],
) -> Result<(), Box<dyn std::error::Error>> {
    let id = rx.commands().id(key)?;
    let def = rx.commands().def(id).ok_or("no command")?.clone();
    let mut record = def.new_args();
    for (k, x) in args {
        let aid = def.args.id(k)?;
        let typed = match x {
            Json::String(t) => Value::Text(t.clone()),
            Json::Bool(b) => Value::Bool(*b),
            Json::Number(n) => Value::F64(n.as_f64().unwrap_or(f64::NAN)),
            _ => Value::None,
        };
        record.set_value(sdrkit_core::FieldId(aid.0), &typed)?;
    }
    owner::invoke(rx, id, &record)?;
    Ok(())
}

/// `seconds` of `gen` through the standard impairments that the `gen_*` settings describe.
pub fn impair(gen: Box<dyn Generator>, seconds: f64, settings: &[Set]) -> Vec<Complex32> {
    let rate = gen.rate_hz();
    let schema = with_generator_settings(&Schema::of(&[])).expect("the generator settings");
    let v = values(&schema, settings);
    let mut g = sdrkit_components::sim::impaired(gen, &v).expect("impairment settings");
    let mut x = vec![Complex32::default(); (seconds * rate).round() as usize];
    let n = g.generate(&mut x);
    x.truncate(n);
    x
}

/// Noise stated as an SNR in the declared bandwidth of the signal.
pub fn snr(db: f64) -> [Set<'static>; 2] {
    [
        ("gen_noise", Json::from("snr")),
        ("gen_snr_db", Json::from(db)),
    ]
}

/// The value of field `key`, if the results declare it and it holds one.
fn value<'a>(r: &'a Results, key: &str) -> Option<&'a Value> {
    let id = r.decl().field_id(key)?;
    r.fields().value(id).filter(|v| !matches!(v, Value::None))
}

/// The number of a value (`NaN` for no number).
fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x)) => *x,
        Some(Value::I64(x)) => *x as f64,
        _ => f64::NAN,
    }
}

/// The text of field `id` of `rec`: a choice by its key, text as it is, a number as people read it.
fn text_of(rec: &Record, key: &str) -> String {
    let Some(id) = rec.decl().id(key) else {
        return String::new();
    };
    match rec.value(id) {
        Some(Value::Choice(i)) => rec
            .decl()
            .field(id)
            .and_then(|d| d.choices.get(usize::from(*i)))
            .map(|c| c.key.to_string())
            .unwrap_or_default(),
        Some(Value::Text(t)) => t.clone(),
        Some(Value::None) | None => String::new(),
        Some(_) => rec.text(id).map(|t| t.to_string()).unwrap_or_default(),
    }
}

/// A number field (`NaN` when missing or without a value).
pub fn num(r: &Results, key: &str) -> f64 {
    number(value(r, key))
}

/// A whole number field (0 when missing).
pub fn int(r: &Results, key: &str) -> i64 {
    match value(r, key) {
        Some(Value::I64(v)) => *v,
        _ => 0,
    }
}

/// A yes or no field (false when missing).
pub fn flag(r: &Results, key: &str) -> bool {
    matches!(value(r, key), Some(Value::Bool(true)))
}

/// A text or choice field (empty when missing). A choice returns its key.
pub fn text(r: &Results, key: &str) -> String {
    text_of(r.fields(), key)
}

/// The label of a choice field (empty when missing).
pub fn label(r: &Results, key: &str) -> String {
    let Some(id) = r.decl().field_id(key) else {
        return String::new();
    };
    r.fields()
        .text(id)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

/// Whether the field holds no value now.
pub fn is_none(r: &Results, key: &str) -> bool {
    value(r, key).is_none()
}

/// Row `row` of the table `def`.
fn row<'a>(r: &'a Results, def: &TableDef, row: &str) -> Option<&'a Record> {
    r.decl()
        .table_id(def.key)
        .and_then(|id| r.table(id))
        .and_then(|t| t.row(row))
}

/// The cell in column `key` of row `row` of the table `def`, or `Value::None`.
pub fn cell(r: &Results, def: &TableDef, row_key: &str, key: &str) -> Value {
    row(r, def, row_key)
        .and_then(|rec| rec.decl().id(key).and_then(|id| rec.value(id)).cloned())
        .unwrap_or(Value::None)
}

/// The text of a cell (empty when missing). A choice returns its key.
pub fn cell_text(r: &Results, def: &TableDef, row_key: &str, key: &str) -> String {
    row(r, def, row_key).map_or_else(String::new, |rec| text_of(rec, key))
}

/// The number of a cell (`NaN` when missing).
pub fn cell_num(r: &Results, def: &TableDef, row_key: &str, key: &str) -> f64 {
    number(Some(&cell(r, def, row_key, key)))
}

/// The keys of the rows of the table `key`, newest first.
pub fn row_ids(r: &Results, key: &str) -> Vec<String> {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map(|t| {
            t.rows_newest_first()
                .map(|(id, _)| id.to_string())
                .collect()
        })
        .unwrap_or_default()
}

struct Counting;

thread_local! {
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

// SAFETY: every call goes straight to the system allocator. The count is a thread-local cell.
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        bump();
        // SAFETY: the caller's layout goes to the system allocator unchanged.
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        // SAFETY: `p` came from `alloc` or `realloc` above with this layout.
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, n: usize) -> *mut u8 {
        bump();
        // SAFETY: as `dealloc`, and `n` is the caller's new size.
        unsafe { std::alloc::System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The allocations that this thread makes while `f` runs.
pub fn allocations(f: impl FnOnce()) -> usize {
    let before = ALLOCS.with(|c| c.get());
    f();
    ALLOCS.with(|c| c.get()) - before
}
