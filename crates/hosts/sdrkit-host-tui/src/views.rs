//! Text rows for the screens, built from the copies of the results. Nothing here draws or calls the library.

use crate::api::{Decl, Value};
use crate::format;
use crate::results::{Results, Row};

/// One stack of the catalog, as the stacks screen shows it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct StackRow {
    /// The stack id.
    pub id: String,
    /// The name.
    pub name: String,
    /// The group.
    pub group: String,
    /// One sentence about the stack.
    pub blurb: String,
    /// The labels of the signal types that the stack receives, comma-separated.
    pub signals: String,
    /// The frequency that the stack starts on, in Hz, or 0.
    pub default_hz: f64,
    /// The sample rate that the stack requests, in Hz, or 0.
    pub rate_hz: f64,
}

/// Returns the cell of column `key` of `row` in table `table`, or `None`.
fn cell<'a>(r: &'a Results, table: usize, row: &'a Row, key: &str) -> Option<&'a Value> {
    row.cells.get(r.column_id(table, key)?)
}

/// Returns the string in `v`, or "".
fn string(v: Option<&Value>) -> &str {
    match v {
        Some(Value::String(s)) => s,
        _ => "",
    }
}

/// Returns the number in `v`, or 0.
fn number(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Hz(x) | Value::F64(x)) => *x,
        Some(Value::I64(i)) => *i as f64,
        _ => 0.0,
    }
}

/// Returns the stacks of the tables `stacks` and `stack_signals` of the engine's results, in table order.
pub fn stacks(engine: &Results) -> Vec<StackRow> {
    let Some(t) = engine.table_id("stacks") else {
        return Vec::new();
    };
    let signals = engine.table_id("stack_signals");
    let mut out: Vec<StackRow> = engine.tables[t]
        .rows
        .iter()
        .map(|row| StackRow {
            id: row.key.clone(),
            name: string(cell(engine, t, row, "name")).to_owned(),
            group: string(cell(engine, t, row, "group")).to_owned(),
            blurb: string(cell(engine, t, row, "blurb")).to_owned(),
            signals: String::new(),
            default_hz: number(cell(engine, t, row, "default_hz")),
            rate_hz: number(cell(engine, t, row, "rate_hz")),
        })
        .collect();
    if let Some(s) = signals {
        let decl = engine
            .column_id(s, "signal")
            .and_then(|c| engine.decl.tables[s].columns.get(c));
        for row in engine.tables[s].rows.iter().rev() {
            let stack = string(cell(engine, s, row, "stack"));
            let (Some(sig), Some(decl)) = (cell(engine, s, row, "signal"), decl) else {
                continue;
            };
            if let Some(st) = out.iter_mut().find(|r| r.id == stack) {
                if !st.signals.is_empty() {
                    st.signals.push_str(", ");
                }
                format::value(&mut st.signals, sig, decl);
            }
        }
    }
    out.sort_by(|a, b| a.group.cmp(&b.group).then_with(|| a.id.cmp(&b.id)));
    out
}

/// Returns the text of each cell of `row` by the declarations `columns`.
pub fn cells(row: &Row, columns: &[Decl]) -> Vec<String> {
    columns
        .iter()
        .zip(&row.cells)
        .map(|(d, v)| format::value_text(v, d))
        .collect()
}

/// Returns the header and the text rows of table `table`, rows `from` to `from + count`, with the row key first.
pub fn table(
    r: &Results,
    table: usize,
    from: usize,
    count: usize,
) -> (Vec<String>, Vec<Vec<String>>) {
    let Some(decl) = r.decl.tables.get(table) else {
        return (Vec::new(), Vec::new());
    };
    let mut header = vec!["key".to_owned()];
    header.extend(decl.columns.iter().map(|c| c.label.clone()));
    let rows = r.tables[table]
        .rows
        .iter()
        .skip(from)
        .take(count)
        .map(|row| {
            let mut line = vec![row.key.clone()];
            line.extend(cells(row, &decl.columns));
            line
        })
        .collect();
    (header, rows)
}

/// Returns the label and the text of each result field.
pub fn fields(r: &Results) -> Vec<(String, String)> {
    r.decl
        .fields
        .iter()
        .zip(&r.fields)
        .map(|(d, v)| (d.label.clone(), format::value_text(v, d)))
        .collect()
}

/// Returns the range of `d` as text, with the unit of the value and what a 0 outside the range means, or "".
pub fn range(d: &Decl) -> String {
    let Some((min, max, step)) = d.range else {
        return String::new();
    };
    let mut s = String::new();
    if d.zero_means.is_empty() {
        format::quantity(&mut s, min, d);
        s.push_str(" to ");
        format::quantity(&mut s, max, d);
    } else if min > 0.0 {
        s.push_str(&d.zero_means);
        s.push_str(", or ");
        format::quantity(&mut s, min, d);
        s.push_str(" to ");
        format::quantity(&mut s, max, d);
    } else {
        s.push_str(&d.zero_means);
        s.push_str(", or up to ");
        format::quantity(&mut s, max, d);
    }
    if step > 0.0 && step != 1.0 {
        s.push_str(" step ");
        format::quantity(&mut s, step, d);
    }
    s
}

/// The value of the result field `key` of `r`, if it has one.
pub fn field<'a>(r: &'a Results, key: &str) -> Option<&'a crate::api::Value> {
    let i = r.decl.fields.iter().position(|d| d.key == key)?;
    r.fields.get(i).filter(|v| !matches!(v, crate::api::Value::None))
}
