//! Markdown pieces shared by the stack pages and the driver pages.

use sdrkit_core::{
    ChoiceDecl, CommandDef, EventDecl, FieldDecl, RecordDecl, ResultsDecl, SettingDecl, Value,
    ValueKind,
};

/// The most options a cell lists before it ends with "and others".
const MAX_OPTIONS: usize = 24;

/// A markdown table. The first row is the header.
pub fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("|{}|\n", vec!["---"; header.len()].join("|")));
    for r in rows {
        let cells: Vec<String> = r.iter().map(|c| cell(c)).collect();
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out
}

/// A table cell: no line breaks and no bare bars.
pub fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

pub fn code(text: &str) -> String {
    format!("`{text}`")
}

/// A number with at most six decimals and no trailing zeros.
pub fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// A frequency or rate in Hz, MHz or kHz.
pub fn hz(v: f64) -> String {
    let a = v.abs();
    if a >= 1e9 {
        format!("{} GHz", num(v / 1e9))
    } else if a >= 1e6 {
        format!("{} MHz", num(v / 1e6))
    } else if a >= 1e3 {
        format!("{} kHz", num(v / 1e3))
    } else {
        format!("{} Hz", num(v))
    }
}

/// A frequency in plain MHz, the form that `--freq` takes.
pub fn mhz(v: f64) -> String {
    num(v / 1e6)
}

pub fn kind_name(f: &FieldDecl) -> String {
    let k = match f.kind {
        ValueKind::Bool => "yes or no",
        ValueKind::I64 => "whole number",
        ValueKind::F64 => "number",
        ValueKind::Text => "text",
        ValueKind::Hz => "frequency",
        ValueKind::Time => "time",
        ValueKind::Duration => "duration",
        ValueKind::Position => "position",
        ValueKind::Choice => "choice",
        ValueKind::Bytes => "bytes",
        ValueKind::Any => "any",
    };
    if f.unit.is_empty() || f.kind == ValueKind::Hz {
        k.to_string()
    } else {
        format!("{k} ({})", f.unit)
    }
}

fn value_text(v: &Value, f: &FieldDecl) -> String {
    match v {
        Value::None => "none".into(),
        Value::Bool(b) => b.to_string(),
        Value::I64(n) => n.to_string(),
        Value::F64(x) => num(*x),
        Value::Text(t) if t.is_empty() => "empty".into(),
        Value::Text(t) => code(t),
        Value::Hz(x) => hz(*x),
        Value::Time(x) | Value::Duration(x) => format!("{} s", num(*x)),
        Value::Position { lat, lon } => format!("{}, {}", num(*lat), num(*lon)),
        Value::Choice(i) => f
            .choices
            .get(usize::from(*i))
            .map_or_else(|| i.to_string(), |c| code(&c.key)),
        Value::Bytes(b) => format!("{} bytes", b.len()),
    }
}

fn options(choices: &[ChoiceDecl]) -> String {
    let mut parts: Vec<String> = choices
        .iter()
        .take(MAX_OPTIONS)
        .map(|c| {
            if c.label == c.key {
                code(&c.key)
            } else {
                format!("{} ({})", code(&c.key), c.label)
            }
        })
        .collect();
    if choices.len() > MAX_OPTIONS {
        parts.push("and others".into());
    }
    parts.join(", ")
}

fn range_text(s: &SettingDecl) -> String {
    if s.field.kind == ValueKind::Choice {
        return options(&s.field.choices);
    }
    match s.range {
        Some(r) if s.field.kind == ValueKind::Hz => format!("{} to {}", hz(r.min), hz(r.max)),
        Some(r) => format!("{} to {}", num(r.min), num(r.max)),
        None => String::new(),
    }
}

pub const SETTING_HEADER: [&str; 6] = [
    "Key",
    "Label",
    "Kind",
    "Default",
    "Range or options",
    "Help",
];

/// The rows of a settings table, one per setting, in the order of the schema.
pub fn setting_rows(settings: &[SettingDecl]) -> Vec<Vec<String>> {
    settings
        .iter()
        .map(|s| {
            vec![
                code(&s.field.key),
                s.field.label.to_string(),
                kind_name(&s.field),
                value_text(&s.default, &s.field),
                range_text(s),
                s.help.to_string(),
            ]
        })
        .collect()
}

/// The tables of `settings`: the settings, the advanced settings, and the settings of the test signal generator
/// (`gen_*`), each group under its own line.
pub fn settings_table(settings: &[SettingDecl]) -> String {
    let (generator, receiver): (Vec<SettingDecl>, Vec<SettingDecl>) = settings
        .iter()
        .cloned()
        .partition(|s| sdrkit_dev_interface::generator_setting(&s.field.key).is_some());
    let (advanced, basic): (Vec<SettingDecl>, Vec<SettingDecl>) =
        receiver.into_iter().partition(|s| s.advanced);
    let mut out = String::new();
    if !basic.is_empty() || advanced.is_empty() {
        out.push_str(&table(&SETTING_HEADER, &setting_rows(&basic)));
    }
    if !advanced.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("Advanced settings. The owner works without any of them set.\n\n");
        out.push_str(&table(&SETTING_HEADER, &setting_rows(&advanced)));
    }
    if !generator.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(GENERATOR_SETTINGS_LEAD);
        out.push_str("\n\n");
        out.push_str(&table(&SETTING_HEADER, &setting_rows(&generator)));
    }
    // What a setting with a role chooses.
    let roles: Vec<String> = settings
        .iter()
        .filter_map(sdrkit_dev_interface::marks::setting_sentence)
        .chain(
            settings
                .iter()
                .filter_map(sdrkit_dev_interface::marks::setting_refers_sentence),
        )
        .collect();
    if !roles.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&roles.join(" "));
        out.push('\n');
    }
    out
}

/// The line above the table of the settings of the test signal generator.
pub const GENERATOR_SETTINGS_LEAD: &str =
    "Settings of the test signal generator. They change the signal of the `synth:` source of the stack.";

fn field_line(f: &FieldDecl) -> String {
    format!("{} ({})", code(&f.key), kind_name(f))
}

pub fn field_list(r: &RecordDecl) -> String {
    r.fields()
        .iter()
        .map(field_line)
        .collect::<Vec<_>>()
        .join(", ")
}

/// A table of fields: key, label, kind.
pub fn fields_table(r: &RecordDecl) -> String {
    let rows: Vec<Vec<String>> = r
        .fields()
        .iter()
        .map(|f| vec![code(&f.key), f.label.to_string(), kind_name(f)])
        .collect();
    table(&["Key", "Label", "Kind"], &rows)
}

/// The sections of a results declaration: fields, tables, series, rasters and map points. A section with nothing in it is left
/// out.
pub fn results_sections(r: &ResultsDecl, fields_title: &str) -> String {
    let mut out = String::new();
    if !r.fields().is_empty() {
        out.push_str(&format!("## {fields_title}\n\n"));
        out.push_str(&fields_table(r.fields()));
        out.push('\n');
    }
    if !r.tables().is_empty() {
        out.push_str("## Result tables\n\n");
        let rows: Vec<Vec<String>> = r
            .tables()
            .iter()
            .map(|t| vec![code(&t.key), t.label.to_string(), field_list(&t.columns)])
            .collect();
        out.push_str(&table(&["Key", "Label", "Columns"], &rows));
        out.push('\n');
    }
    if !r.series_decls().is_empty() {
        out.push_str("## Result series\n\n");
        let rows: Vec<Vec<String>> = r
            .series_decls()
            .iter()
            .map(|s| {
                vec![
                    code(&s.key),
                    s.label.clone(),
                    s.x_unit.clone(),
                    s.y_unit.clone(),
                ]
            })
            .collect();
        out.push_str(&table(&["Key", "Label", "X unit", "Y unit"], &rows));
        out.push('\n');
    }
    if !r.rasters().is_empty() {
        out.push_str("## Result pictures\n\n");
        let rows: Vec<Vec<String>> = r
            .rasters()
            .iter()
            .map(|s| {
                vec![
                    code(&s.key),
                    s.label.clone(),
                    s.x_unit.clone(),
                    s.y_unit.clone(),
                    s.unit.clone(),
                ]
            })
            .collect();
        out.push_str(&table(
            &["Key", "Label", "X unit", "Y unit", "Cell unit"],
            &rows,
        ));
        out.push('\n');
    }
    if !r.points_label().is_empty() {
        out.push_str(&format!(
            "## Map points\n\nThe stack puts points on a map: {}.\n\n",
            r.points_label()
        ));
    }
    out
}

pub fn events_section(events: &[EventDecl]) -> String {
    let rows: Vec<Vec<String>> = events
        .iter()
        .map(|e| vec![code(&e.kind), e.label.to_string(), field_list(&e.fields)])
        .collect();
    format!(
        "## Events\n\n{}\n",
        table(&["Kind", "Label", "Fields"], &rows)
    )
}

fn argument_line(a: &SettingDecl) -> String {
    let rest = if a.required {
        "required".to_string()
    } else {
        format!("default {}", value_text(&a.default, &a.field))
    };
    format!("{} ({}, {rest})", code(&a.field.key), kind_name(&a.field))
}

/// A table of commands: key, label, arguments, reply, help.
pub fn commands_table(defs: &[CommandDef]) -> String {
    let rows: Vec<Vec<String>> = defs
        .iter()
        .map(|c| {
            vec![
                code(&c.key),
                c.label.to_string(),
                c.args
                    .settings()
                    .iter()
                    .map(argument_line)
                    .collect::<Vec<_>>()
                    .join(", "),
                field_list(&c.reply),
                c.help.to_string(),
            ]
        })
        .collect();
    table(&["Key", "Label", "Arguments", "Reply", "Help"], &rows)
}
