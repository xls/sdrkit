//! Helpers the tests of the family share: typed reads of results and tables, and settings in the settings JSON form.

use sdrkit_core::verify::CaseSetting;
use sdrkit_core::{owner, Receiver, Record, Results, SettingError, StackDescriptor, Value, Values};
use sdrkit_dev_test::Run;
use serde_json::{json, Value as Json};

/// A number field of a run, as a whole number (0 if it is missing).
pub fn int(r: &Run, key: &str) -> u64 {
    match r.field(key) {
        Some(Value::I64(n)) => (*n).max(0) as u64,
        Some(Value::F64(x) | Value::Hz(x) | Value::Duration(x)) => x.max(0.0) as u64,
        _ => 0,
    }
}

/// A value as a number, for the number kinds.
pub fn number(v: &Value) -> Option<f64> {
    match v {
        Value::I64(n) => Some(*n as f64),
        Value::F64(x) | Value::Hz(x) | Value::Duration(x) | Value::Time(x) => Some(*x),
        _ => None,
    }
}

/// A text field of results, or a choice field as its key ("" if it is missing).
pub fn field_text(r: &Results, key: &str) -> String {
    let Some(id) = r.decl().field_id(key) else {
        return String::new();
    };
    let choices = r.decl().fields().field(id).map(|d| d.choices.clone());
    match r.fields().value(id) {
        Some(Value::Text(s)) => s.to_string(),
        Some(Value::Choice(i)) => choices
            .and_then(|c| c.get(usize::from(*i)).map(|c| c.key.to_string()))
            .unwrap_or_default(),
        Some(v) => number(v).map(|x| x.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// The rows of table `key`, newest first (empty if the table is not declared).
pub fn rows<'a>(r: &'a Results, key: &str) -> Vec<(&'a str, &'a Record)> {
    match r.decl().table_id(key).and_then(|id| r.table(id)) {
        Some(t) => t.rows_newest_first().collect(),
        None => Vec::new(),
    }
}

/// The number of rows of table `key`.
pub fn row_count(r: &Results, key: &str) -> usize {
    r.decl()
        .table_id(key)
        .and_then(|id| r.table(id))
        .map_or(0, |t| t.len())
}

/// Cell `key` of a row as text, a choice as its key, a number as its text ("" if it is empty).
pub fn cell(row: &Record, key: &str) -> String {
    let decl = row.decl();
    let Some(id) = decl.id(key) else {
        return String::new();
    };
    match row.value(id) {
        Some(Value::Text(s)) => s.to_string(),
        Some(Value::Choice(i)) => decl
            .field(id)
            .and_then(|d| d.choices.get(usize::from(*i)).map(|c| c.key.to_string()))
            .unwrap_or_default(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Position { lat, lon }) => format!("{lat},{lon}"),
        Some(v) => number(v).map(|x| x.to_string()).unwrap_or_default(),
        None => String::new(),
    }
}

/// Cell `key` of a row as a number.
pub fn cell_num(row: &Record, key: &str) -> Option<f64> {
    let id = row.decl().id(key)?;
    number(row.value(id)?)
}

/// The settings of `d` (its own, then the generator settings) with `set` applied.
pub fn values(d: &StackDescriptor, set: &[CaseSetting]) -> Values {
    let schema = sdrkit_components::sim::with_generator_settings(&(d.settings)())
        .expect("the settings take the generator settings");
    let mut v = Values::defaults(&schema);
    CaseSetting::apply_all(set, &mut v).unwrap_or_else(|e| panic!("{}: {e:?}", d.id));
    v
}

/// Applies setting `key` of `rx` with `value` in the settings JSON form, through the checked path.
pub fn set(rx: &mut dyn Receiver, key: &str, value: Json) -> Result<(), SettingError> {
    let schema = rx.schema().clone();
    let id = schema.id(key)?;
    let mut v = Values::defaults(&schema);
    v.update_from_json(&json!({ key: value }))
        .map_err(|e| match e {
            sdrkit_core::ValuesError::Setting(s) => s,
            _ => SettingError::Malformed { id, at: None },
        })?;
    let x = v.value(id).cloned().unwrap_or(Value::None);
    owner::apply(rx, id, &x)
}

/// Runs command `key` of `rx` with its default arguments, through the checked path.
pub fn command(rx: &mut dyn Receiver, key: &str) -> Result<(), sdrkit_core::CommandError> {
    command_with(rx, key, &[])
}

/// Runs command `key` of `rx` with the arguments `args` (by key) and the defaults for the rest, through the checked path.
pub fn command_with(
    rx: &mut dyn Receiver,
    key: &str,
    args: &[(&str, Value)],
) -> Result<(), sdrkit_core::CommandError> {
    let commands = rx.commands();
    let id = commands.id(key)?;
    let mut given = commands.defs()[id.index()].new_args();
    for (k, v) in args {
        let a = given.decl().id(k).expect("an argument of the command");
        given
            .set_value(a, v)
            .expect("a value of the argument's kind");
    }
    owner::invoke(rx, id, &given).map(|_| ())
}

/// A bytes cell of a row (empty if it is not bytes).
pub fn cell_bytes(row: &Record, key: &str) -> Vec<u8> {
    let Some(id) = row.decl().id(key) else {
        return Vec::new();
    };
    match row.value(id) {
        Some(Value::Bytes(b)) => b.to_vec(),
        _ => Vec::new(),
    }
}
