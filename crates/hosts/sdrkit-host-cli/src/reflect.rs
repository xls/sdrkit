//! The commands that work on any owner through its declarations: the catalogs, `show`, `settings`, `call`, `events`
//! and the C header of key constants.

use crate::error::CliError;
use crate::host::{self, Live};
use crate::out::Out;
use crate::text;
use sdrkit_core::{Owner, Record, ResultsDecl, Schema, Value, ValueKind};
use sdrkit_host_engine::{system_clock, Engine, OwnerRef};
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

/// How often `--follow` reads the owner again.
const FOLLOW: Duration = Duration::from_millis(500);

/// The wall time, seconds since 1970.
pub fn now() -> f64 {
    system_clock().now()
}

/// A table of an owner, made text or JSON inside the owner's lock.
pub struct TableText {
    /// The column headings: the row key, then the column labels.
    pub headings: Vec<String>,
    /// The rows that changed after the requested counter, newest first.
    pub rows: Vec<Vec<String>>,
    /// The same rows as `[key, record]` pairs.
    pub json: Vec<serde_json::Value>,
    /// The change counter of the table now.
    pub counter: u64,
}

/// The rows of table `key` of `o` that changed after `since` (0 for all). `None` if `o` declares no such table.
pub fn table_text(o: &dyn Owner, key: &str, since: u64, json: bool) -> Option<TableText> {
    let r = o.results();
    let t = r.table(r.decl().table_id(key)?)?;
    let cols = t.columns();
    let mut headings = vec!["Key".to_owned()];
    headings.extend(cols.fields().iter().map(|f| match f.unit.as_ref() {
        "" => f.label.to_string(),
        _ if matches!(
            f.kind,
            ValueKind::Hz | ValueKind::Duration | ValueKind::Time
        ) =>
        {
            f.label.to_string()
        }
        u => format!("{} ({u})", f.label),
    }));
    let now = now();
    let mut rows = Vec::new();
    let mut js = Vec::new();
    let changed: Vec<(&str, &Record)> = if since == 0 {
        t.rows_newest_first().collect()
    } else {
        t.rows_changed_since(since).collect()
    };
    for (k, rec) in changed {
        if json {
            js.push(json!([k, rec]));
        } else {
            let mut row = vec![k.to_owned()];
            row.extend(rec.iter().map(|(d, v)| cell_plain(d, v, now)));
            rows.push(row);
        }
    }
    Some(TableText {
        headings,
        rows,
        json: js,
        counter: t.change_counter(),
    })
}

/// A cell: the value by its declaration, without the unit that the heading already carries.
fn cell_plain(d: &sdrkit_core::FieldDecl, v: &Value, now: f64) -> String {
    let s = text::cell(d, v, now);
    let unit = d.unit.as_ref();
    let kept = matches!(v, Value::Hz(_) | Value::Duration(_) | Value::Time(_))
        || unit == "Hz"
        || unit == "s";
    match s.strip_suffix(unit) {
        Some(rest) if !unit.is_empty() && !kept => rest.trim_end().to_owned(),
        _ => s,
    }
}

/// Prints table `key` of owner `r`, the rows changed after `since`, and returns the table's change counter.
///
/// # Errors
///
/// [`CliError::UnknownTable`] if the owner declares no such table, what the engine returns.
pub fn print_table(
    engine: &Engine,
    r: OwnerRef,
    name: &str,
    key: &str,
    since: u64,
    out: &mut Out,
) -> Result<u64, CliError> {
    let json = out.json;
    let t = engine
        .owner(r, |o| table_text(o, key, since, json))?
        .ok_or_else(|| CliError::UnknownTable {
            owner: name.to_owned(),
            table: key.to_owned(),
        })?;
    if json {
        out.json(&json!({ "owner": name, "table": key, "counter": t.counter, "rows": t.json }))?;
    } else if !t.rows.is_empty() || since == 0 {
        let mut s = String::new();
        text::table(&t.headings, &t.rows, &mut s);
        out.text(&s)?;
    }
    Ok(t.counter)
}

/// The marks of table `t` as text after its line, each with `; ` before it: the column that names a row, the default
/// order and the columns of the place of a row. Empty for a table without marks.
fn table_marks(t: &sdrkit_core::TableDecl) -> String {
    let m = &t.marks;
    let key = |c: u16| t.columns.fields()[usize::from(c)].key.as_ref();
    let mut out = String::new();
    if let Some(c) = m.label {
        let _ = write!(out, "; label {}", key(c));
    }
    if let Some((c, order)) = m.sort {
        let order = match order {
            sdrkit_core::SortOrder::Ascending => "ascending",
            sdrkit_core::SortOrder::Descending => "descending",
        };
        let _ = write!(out, "; sort {} {order}", key(c));
    }
    for (name, c) in [
        ("position", m.position),
        ("latitude", m.latitude),
        ("longitude", m.longitude),
        ("altitude", m.altitude),
        ("course", m.course),
        ("speed", m.speed),
    ] {
        if let Some(c) = c {
            let _ = write!(out, "; {name} {}", key(c));
        }
    }
    out
}

/// The fields, the status, the log and the declared tables of `o`, as text, with the fields changed after `since`.
fn results_text(o: &dyn Owner, since: u64, now: f64) -> String {
    let r = o.results();
    let mut s = String::new();
    let st = r.status();
    let _ = write!(s, "Status: {:?}", st.lock_state);
    if let Some(v) = st.snr_db {
        let _ = write!(s, ", SNR {v:.1} dB");
    }
    if let Some(v) = st.level_dbfs {
        let _ = write!(s, ", level {v:.1} dBFS");
    }
    if let Some(v) = st.offset_hz {
        let _ = write!(s, ", offset {}", text::hz(v));
    }
    if !st.summary.is_empty() {
        let _ = write!(s, ", {}", st.summary);
    }
    s.push('\n');
    if since == 0 || r.fields_counter() > since {
        for (d, v) in r.fields().iter().filter(|(_, v)| !v.is_none()) {
            let _ = writeln!(s, "{}: {}", d.label, text::cell(d, v, now));
        }
    }
    let decl: &ResultsDecl = r.decl();
    for (i, t) in decl.tables().iter().enumerate() {
        let rows = u16::try_from(i)
            .ok()
            .and_then(|i| r.table(sdrkit_core::TableId(i)))
            .map_or(0, sdrkit_core::Table::len);
        let _ = writeln!(
            s,
            "Table {} ({}): {rows} rows{}",
            t.key,
            t.label,
            table_marks(t)
        );
    }
    for x in decl.series_decls() {
        let _ = writeln!(s, "Series {} ({})", x.key, x.label);
    }
    for x in decl.rasters() {
        let _ = writeln!(s, "Raster {} ({})", x.key, x.label);
    }
    for l in r.log_lines() {
        let _ = writeln!(s, "| {l}");
    }
    s
}

/// `show`: the results of owner `name`, or one table, changed after `since`. With `follow`, prints the changes until
/// that function returns true.
///
/// # Errors
///
/// [`CliError::UnknownOwner`], [`CliError::UnknownTable`], what the engine returns.
pub fn show(
    engine: &Engine,
    live: Option<&Live>,
    name: &str,
    table: Option<&str>,
    mut since: u64,
    follow: Option<&dyn Fn() -> bool>,
    out: &mut Out,
) -> Result<(), CliError> {
    let r = host::owner(engine, name, live)?;
    loop {
        let counter = match table {
            Some(t) => print_table(engine, r, name, t, since, out)?,
            None => {
                let json = out.json;
                let now = now();
                let (counter, body) = engine.owner(r, |o| {
                    let res = o.results();
                    let body = if json {
                        serde_json::to_value(res).map_err(CliError::from)
                    } else {
                        Ok(serde_json::Value::String(results_text(o, since, now)))
                    };
                    (res.change_counter(), body)
                })?;
                if counter > since || since == 0 {
                    match body? {
                        serde_json::Value::String(s) => out.text(&s)?,
                        v => {
                            out.json(&json!({ "owner": name, "counter": counter, "results": v }))?
                        }
                    }
                }
                counter
            }
        };
        let Some(stop) = follow.filter(|stop| !stop()) else {
            return Ok(());
        };
        since = counter;
        std::thread::sleep(FOLLOW);
        if stop() {
            return Ok(());
        }
    }
}

/// The schema of `o` as a table: key, label, kind, range, choices, value, help.
fn schema_rows(schema: &Schema, values: Option<&sdrkit_core::Values>) -> Vec<Vec<String>> {
    let now = now();
    schema
        .settings()
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let f = &s.field;
            let id = sdrkit_core::SettingId(u16::try_from(i).unwrap_or(u16::MAX));
            let v = values.and_then(|v| v.value(id)).unwrap_or(&s.default);
            let range = s.range.map_or(String::new(), |r| {
                let n = |x: f64| text::cell(f, &Value::F64(x), now);
                format!("{} to {}", n(r.min), n(r.max))
            });
            let common: Vec<String> = schema
                .common(id, values)
                .iter()
                .map(|&x| text::cell(f, &Value::F64(x), now))
                .collect();
            let choices: Vec<&str> = f.choices.iter().map(|c| c.key.as_ref()).collect();
            let choices = if choices.len() > 8 {
                format!(
                    "{} ... ({} choices)",
                    choices[..8].join(", "),
                    choices.len()
                )
            } else {
                choices.join(", ")
            };
            vec![
                f.key.to_string(),
                f.label.to_string(),
                format!("{:?}", f.kind).to_lowercase(),
                range,
                common.join(", "),
                choices,
                text::cell(f, v, now),
                s.help.to_string(),
            ]
        })
        .collect()
}

/// `settings <owner> [key=value...]`: applies the values given, then prints the schema with the values. The settings of
/// the engine and the services are kept for the next run.
///
/// # Errors
///
/// [`CliError::UnknownOwner`], [`CliError::UnknownSetting`], [`CliError::BadValue`], what the owner returns.
pub fn settings(
    engine: &Engine,
    storage: &dyn sdrkit_core::Storage,
    live: Option<&Live>,
    name: &str,
    assignments: &[String],
    out: &mut Out,
) -> Result<(), CliError> {
    let r = host::owner(engine, name, live)?;
    if !assignments.is_empty() {
        apply(engine, r, name, assignments)?;
        if matches!(r, OwnerRef::Engine | OwnerRef::Service(_)) && live.is_none() {
            host::save_settings(engine, storage)?;
        }
    }
    let json = out.json;
    let (rows, js) = engine.owner(r, |o| {
        let js =
            json.then(|| json!({ "owner": name, "settings": o.schema(), "values": o.values() }));
        (schema_rows(o.schema(), o.values()), js)
    })?;
    match js {
        Some(v) => out.json(&v),
        None => {
            let mut s = String::new();
            let h = [
                "Key", "Label", "Kind", "Range", "Common", "Choices", "Value", "Help",
            ]
            .map(String::from);
            text::table(&h, &rows, &mut s);
            out.text(&s)
        }
    }
}

/// Applies `key=value` texts to owner `r`, each read by the kind of its setting.
///
/// # Errors
///
/// [`CliError::NotKeyValue`], [`CliError::UnknownSetting`], [`CliError::BadValue`], what the owner returns.
pub fn apply(
    engine: &Engine,
    r: OwnerRef,
    name: &str,
    assignments: &[String],
) -> Result<(), CliError> {
    engine.owner(r, |o| -> Result<(), CliError> {
        for a in assignments {
            let (k, v) = text::key_value(a)?;
            let id = o.schema().id(k).map_err(|_| CliError::UnknownSetting {
                owner: format!("`{name}`"),
                key: k.to_owned(),
            })?;
            let decl = o
                .schema()
                .setting(id)
                .map(|s| s.field.clone())
                .ok_or_else(|| {
                    sdrkit_host_engine::EngineError::setting(
                        o.schema(),
                        sdrkit_core::SettingError::NoSuchId { id },
                    )
                })?;
            let value = text::parse_value(&decl, v)?;
            sdrkit_host_engine::EngineError::apply(o, id, &value)?;
        }
        Ok(())
    })?
}

/// The commands of `o` with their arguments, as text.
fn commands_text(o: &dyn Owner) -> String {
    let mut s = String::new();
    for d in o.commands().defs() {
        let args: Vec<String> = d
            .args
            .settings()
            .iter()
            .map(|a| {
                let k = &a.field.key;
                if a.required {
                    format!("{k}=<{:?}>", a.field.kind).to_lowercase()
                } else {
                    format!("[{k}=...]")
                }
            })
            .collect();
        let _ = writeln!(s, "{} {}  {}", d.key, args.join(" "), d.label);
        if !d.help.is_empty() {
            let _ = writeln!(s, "    {}", d.help);
        }
    }
    s
}

/// What a call returned: the reply as text, or the owner's commands when it has no such command.
enum Reply {
    Done(String),
    Unknown(String),
}

/// `call <owner> <command> [key=value...]`: runs the command with the arguments read by their declarations, and prints
/// the reply. Without a command, lists the commands of the owner.
///
/// # Errors
///
/// [`CliError::UnknownOwner`], [`CliError::Command`] (an unknown command lists the owner's commands first),
/// [`CliError::BadValue`].
pub fn call(
    engine: &Engine,
    live: Option<&Live>,
    name: &str,
    command: Option<&str>,
    args: &[String],
    out: &mut Out,
) -> Result<(), CliError> {
    let r = host::owner(engine, name, live)?;
    let Some(command) = command else {
        if out.json {
            let list = engine.owner(r, |o| {
                o.commands()
                    .defs()
                    .iter()
                    .map(|d| json!({ "key": d.key, "label": d.label, "help": d.help, "args": d.args, "reply": d.reply }))
                    .collect::<Vec<_>>()
            })?;
            return out.json(&json!({ "owner": name, "commands": list }));
        }
        let s = engine.owner(r, |o| commands_text(o))?;
        return out.text(&s);
    };
    let now = now();
    let json = out.json;
    let reply = engine.owner(r, |o| -> Result<Reply, CliError> {
        let id = match o.commands().id(command) {
            Ok(id) => id,
            Err(_) => return Ok(Reply::Unknown(commands_text(o))),
        };
        let Some(def) = o.commands().def(id) else {
            return Ok(Reply::Unknown(commands_text(o)));
        };
        let mut rec = def.new_args();
        for a in args {
            let (k, v) = text::key_value(a)?;
            let unknown = || CliError::UnknownSetting {
                owner: format!("the command `{command}` of `{name}`"),
                key: k.to_owned(),
            };
            let fid = rec.decl().id(k).ok_or_else(unknown)?;
            let decl = rec.decl().field(fid).cloned().ok_or_else(unknown)?;
            let value = text::parse_value(&decl, v)?;
            rec.set_value(fid, &value).map_err(|_| CliError::BadValue {
                key: k.to_owned(),
                text: v.to_owned(),
                kind: decl.kind,
                forms: String::new(),
            })?;
        }
        let reply = sdrkit_host_engine::EngineError::invoke(o, id, &rec)?;
        Ok(Reply::Done(match (reply, json) {
            (None, true) => json!({ "owner": name, "command": command, "reply": null }).to_string(),
            (Some(r), true) => json!({ "owner": name, "command": command, "reply": r }).to_string(),
            (None, false) => "done\n".to_owned(),
            (Some(r), false) => {
                let mut s = String::new();
                for (d, v) in r.iter() {
                    let _ = writeln!(s, "{}: {}", d.label, text::cell(d, v, now));
                }
                s
            }
        }))
    })??;
    match reply {
        Reply::Done(s) if json => out.line(&s),
        Reply::Done(s) => out.text(&s),
        Reply::Unknown(list) => {
            out.note(&format!("the commands of `{name}`:\n{list}"))?;
            Err(sdrkit_host_engine::EngineError::no_command(command).into())
        }
    }
}

/// Prints the events that wait at owner `r`.
///
/// # Errors
///
/// What the engine returns.
pub fn print_events(
    engine: &Engine,
    r: OwnerRef,
    name: &str,
    out: &mut Out,
) -> Result<usize, CliError> {
    let mut v = Vec::new();
    engine.take_events(r, &mut v)?;
    let catalog = engine.event_catalog();
    let now = now();
    for e in &v {
        if out.json {
            out.json(&json!({ "owner": name, "event": e }))?;
            continue;
        }
        let label = e
            .kind_id()
            .and_then(|id| catalog.def(id))
            .map_or_else(|| e.kind().to_owned(), |d| d.label.to_string());
        let mut s = format!("[{}] {label}", e.kind());
        if e.freq_hz > 0.0 {
            let _ = write!(s, " on {}", text::hz(e.freq_hz));
        }
        for (d, val) in e.fields.iter().filter(|(_, v)| !v.is_none()) {
            let _ = write!(s, ", {}: {}", d.label, text::cell(d, val, now));
        }
        out.line(&s)?;
    }
    Ok(v.len())
}

/// `events <owner>`: the events that wait, and with `follow` the next ones until `stop` returns true.
///
/// # Errors
///
/// [`CliError::UnknownOwner`], what the engine returns.
pub fn events(
    engine: &Engine,
    live: Option<&Live>,
    name: &str,
    follow: bool,
    out: &mut Out,
    stop: &dyn Fn() -> bool,
) -> Result<(), CliError> {
    let r = host::owner(engine, name, live)?;
    loop {
        print_events(engine, r, name, out)?;
        if !follow || stop() {
            return Ok(());
        }
        std::thread::sleep(FOLLOW);
    }
}

/// Every owner whose declarations an app can know before anything runs: the engine, the services and every kind of the
/// catalogs, with their owner keys.
fn declared_owners(engine: &Engine) -> Result<Vec<(String, OwnerRef)>, CliError> {
    let mut v = vec![("engine".to_owned(), OwnerRef::Engine)];
    let tables = [
        (sdrkit_host_engine::SERVICES.key, ""),
        (sdrkit_host_engine::STACKS.key, "stack."),
        (sdrkit_host_engine::DRIVERS.key, "driver."),
        (sdrkit_host_engine::COMPONENT_KINDS.key, "component."),
    ];
    for (table, prefix) in tables {
        let keys = engine.owner(OwnerRef::Engine, |o| host::table_keys(o, table))?;
        for k in keys {
            let key = format!("{prefix}{k}");
            if let Ok(r) = engine.find(&key) {
                v.push((key, r));
            }
        }
    }
    Ok(v)
}

/// A C name: upper case, every other character `_`.
fn c_name(parts: &[&str]) -> String {
    let mut s = String::from("SDRKIT");
    for p in parts {
        s.push('_');
        s.extend(p.chars().map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        }));
    }
    s
}

/// The key constants of one owner, added to `defs`.
fn owner_constants(key: &str, o: &dyn Owner, defs: &mut BTreeMap<String, String>) {
    let mut add = |name: String, value: &str| {
        let mut n = name;
        while defs.get(&n).is_some_and(|v| v != value) {
            n.push('_');
        }
        defs.insert(n, value.to_owned());
    };
    add(c_name(&[key]), key);
    for s in o.schema().settings() {
        add(c_name(&[key, "setting", &s.field.key]), &s.field.key);
    }
    let r = o.results_decl();
    for f in r.fields().fields() {
        add(c_name(&[key, "field", &f.key]), &f.key);
    }
    for t in r.tables() {
        add(c_name(&[key, "table", &t.key]), &t.key);
        for c in t.columns.fields() {
            add(c_name(&[key, "table", &t.key, "column", &c.key]), &c.key);
        }
    }
    for x in r.series_decls() {
        add(c_name(&[key, "series", &x.key]), &x.key);
    }
    for x in r.rasters() {
        add(c_name(&[key, "raster", &x.key]), &x.key);
    }
    for e in o.events() {
        add(c_name(&["event", &e.kind]), &e.kind);
        for f in e.fields.fields() {
            add(c_name(&["event", &e.kind, "field", &f.key]), &f.key);
        }
    }
    for c in o.commands().defs() {
        add(c_name(&[key, "command", &c.key]), &c.key);
        for a in c.args.settings() {
            add(
                c_name(&[key, "command", &c.key, "arg", &a.field.key]),
                &a.field.key,
            );
        }
        for f in c.reply.fields() {
            add(c_name(&[key, "command", &c.key, "reply", &f.key]), &f.key);
        }
    }
}

/// `declarations --c-header`: a C header with a `#define` of each key of every declared owner: settings, fields,
/// tables and columns, series, rasters, events and their fields, commands with their arguments and replies.
///
/// # Errors
///
/// What the engine returns. A stack kind that cannot be made is left out with a note.
pub fn c_header(engine: &Engine, out: &mut Out) -> Result<String, CliError> {
    let mut defs = BTreeMap::new();
    for (key, r) in declared_owners(engine)? {
        if let Err(e) = engine.owner(r, |o| owner_constants(&key, o, &mut defs)) {
            out.note(&format!("{key} is left out: {e}"))?;
        }
    }
    let mut s = String::from(
        "/* The keys of the settings, results, events and commands of every sdrkit owner. Written by `sdrkit declarations --c-header`. */\n\
         #ifndef SDRKIT_KEYS_H\n#define SDRKIT_KEYS_H\n\n",
    );
    for (name, value) in &defs {
        let v: String = value
            .chars()
            .flat_map(|c| match c {
                '"' | '\\' => vec!['\\', c],
                c if c.is_control() => Vec::new(),
                c => vec![c],
            })
            .collect();
        let _ = writeln!(s, "#define {name} \"{v}\"");
    }
    s.push_str("\n#endif\n");
    Ok(s)
}

/// Prints a catalog table of the engine, sorted by column `sort` if given, with only the columns `keep` if it is not
/// empty.
///
/// # Errors
///
/// What the engine returns.
pub fn catalog(
    engine: &Engine,
    table: &str,
    sort: Option<&str>,
    keep: &[&str],
    out: &mut Out,
) -> Result<(), CliError> {
    let json = out.json;
    let (t, col, cols) = engine.owner(OwnerRef::Engine, |o| {
        let r = o.results();
        let decl = r
            .decl()
            .table_id(table)
            .and_then(|id| r.table(id))
            .map(|t| t.columns().clone());
        let index = |k: &str| decl.as_ref().and_then(|d| d.id(k)).map(|f| f.index() + 1);
        let cols: Vec<usize> = keep.iter().filter_map(|k| index(k)).collect();
        (table_text(o, table, 0, json), sort.and_then(index), cols)
    })?;
    let Some(mut t) = t else {
        return Err(CliError::UnknownTable {
            owner: "engine".into(),
            table: table.into(),
        });
    };
    if json {
        for row in &t.json {
            out.json(row)?;
        }
        return Ok(());
    }
    t.rows.reverse();
    if let Some(c) = col {
        t.rows
            .sort_by(|a, b| a.get(c).cmp(&b.get(c)).then_with(|| a[0].cmp(&b[0])));
    }
    let pick = |row: &[String]| -> Vec<String> {
        if cols.is_empty() {
            return row.to_vec();
        }
        std::iter::once(0)
            .chain(cols.iter().copied())
            .filter_map(|i| row.get(i).cloned())
            .collect()
    };
    let headings = pick(&t.headings);
    let rows: Vec<Vec<String>> = t.rows.iter().map(|r| pick(r)).collect();
    let mut s = String::new();
    text::table(&headings, &rows, &mut s);
    out.text(&s)
}
