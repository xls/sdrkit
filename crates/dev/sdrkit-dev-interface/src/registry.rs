//! The registry record of a family: one line for each stack, shape, field, table, series, event kind, setting and
//! command, with its type, unit and the version in which it appeared. [`compare`] says what changed between two
//! records, and [`verdict`] says whether the version of the new record permits the change.
//!
//! A line is the path of the item, then `name=value` pairs, with one space between them. A space in a value is
//! written as `%20`.

use crate::{shapes, type_name};
use sdrkit_core::interface::{Family, Version};
use sdrkit_core::{FieldDecl, RecordDecl};
use std::collections::BTreeMap;
use std::fmt::Write;

fn value(v: &str) -> String {
    v.replace(' ', "%20")
}

fn field_list(fields: &[FieldDecl]) -> String {
    let parts: Vec<String> = fields
        .iter()
        .map(|f| format!("{}:{}", f.key, type_name(f)))
        .collect();
    value(&parts.join(";"))
}

/// The registry record of `family`.
pub fn render(family: &Family) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# The interface of the stack family {}. The registry test of the family writes this file.",
        family.key
    );
    let _ = writeln!(out, "family {} version={}", family.key, family.version);
    for u in &family.uses {
        let path = format!("uses.{}", u.key);
        let _ = writeln!(
            out,
            "{path} major={} since={}",
            u.version.major,
            family.since(&path)
        );
    }
    for (shape, _) in shapes(family) {
        let path = format!("shape.{}", shape.name);
        let _ = writeln!(out, "{path} since={}", family.since(&path));
        for (f, note) in shape.fields.iter().zip(shape.notes) {
            let d = f.decl();
            let path = format!("{path}.{}", d.key);
            let _ = write!(
                out,
                "{path} type={} unit={} optional={}",
                type_name(&d),
                value(&d.unit),
                if note.optional { "yes" } else { "no" }
            );
            if let Some(r) = &d.refers_to {
                let _ = write!(out, " refers_to={}", value(&r.table));
            }
            // The place of the field among the key readings of its owner.
            if d.reading > 0 {
                let _ = write!(out, " reading={}", d.reading);
            }
            let _ = writeln!(out, " since={}", family.since(&path));
        }
    }
    for stack in &family.stacks {
        let id = stack.id;
        let since = |path: &str| family.since(path);
        let _ = writeln!(out, "{id} since={}", since(id));
        // The kind of content and the media of the stack.
        if let Some(marks) = crate::marks::stack_attributes(stack) {
            let path = format!("{id}.content");
            let _ = writeln!(out, "{path}{marks} since={}", since(&path));
        }
        for f in &stack.fields {
            let path = format!("{id}.fields.{}", f.name);
            // A set that the receiver writes itself says so.
            let writer = if stack.receiver_sets.contains(&f.name) {
                " writer=receiver"
            } else {
                ""
            };
            let _ = writeln!(out, "{path}{writer} since={}", since(&path));
        }
        for c in &stack.components {
            let path = match c.shape {
                Some(f) => format!("{id}.component.{}.fields.{}", c.component, f.name),
                None => format!("{id}.component.{}.dropped", c.component),
            };
            // The fields that the component writes under the key of the stack.
            let shared = if c.shared.is_empty() {
                String::new()
            } else {
                format!(" shared={}", c.shared.join(";"))
            };
            let _ = writeln!(out, "{path}{shared} since={}", since(&path));
        }
        for k in &stack.kept {
            let path = format!("{id}.kept.{k}");
            let _ = writeln!(out, "{path} since={}", since(&path));
        }
        for t in &stack.tables {
            let path = format!("{id}.table.{}", t.def.key);
            // The components that the description names as the writers of a table of the stack.
            let writers = stack
                .table_writers
                .iter()
                .find(|(k, _)| *k == t.def.key)
                .map(|(_, w)| format!(" writers={}", w.join(";")))
                .unwrap_or_default();
            // The column that names a row, the default order and the columns of a place.
            let marks = crate::marks::table_attributes(t.def);
            let _ = writeln!(
                out,
                "{path} row={} cap={}{writers}{marks} since={}",
                t.row.name,
                t.def.cap,
                since(&path)
            );
        }
        for s in &stack.series {
            let path = format!("{id}.series.{}", s.key);
            let _ = writeln!(
                out,
                "{path} x_unit={} y_unit={} since={}",
                value(s.x_unit),
                value(s.y_unit),
                since(&path)
            );
        }
        for r in &stack.rasters {
            let path = format!("{id}.raster.{}", r.key);
            let _ = writeln!(
                out,
                "{path} x_unit={} y_unit={} unit={} since={}",
                value(r.x_unit),
                value(r.y_unit),
                value(r.unit),
                since(&path)
            );
        }
        for e in &stack.events {
            let path = format!("{id}.event.{}", e.def.kind);
            let _ = writeln!(
                out,
                "{path} fields={} since={}",
                e.fields.name,
                since(&path)
            );
        }
        for s in &stack.settings {
            let d = s.decl();
            let path = format!("{id}.setting.{}", d.field.key);
            // What the setting chooses for a front end.
            let role = d
                .role
                .map_or(String::new(), |r| format!(" role={}", r.key()));
            // The table and the column that give values for the setting.
            let refers = crate::marks::value_column(&d.field)
                .map_or(String::new(), |r| format!(" refers={r}"));
            let _ = writeln!(
                out,
                "{path} type={} unit={}{role}{refers} since={}",
                value(&type_name(&d.field)),
                value(&d.field.unit),
                since(&path)
            );
        }
        for c in &stack.commands {
            let def = c.command.def();
            // An optional argument has `?` after its type.
            let args: Vec<String> = def
                .args
                .settings()
                .iter()
                .map(|a| {
                    let mark = if a.required { "" } else { "?" };
                    format!("{}:{}{mark}", a.field.key, type_name(&a.field))
                })
                .collect();
            let path = format!("{id}.command.{}", c.command.key);
            // A command that transmits names what it sends.
            let transmits = def
                .transmits
                .map(|t| format!(" transmits={}", sdrkit_core::Choice::key(t)))
                .unwrap_or_default();
            // A transmission that lasts names the command that stops it.
            let stop = def
                .stopped_by
                .map(|k| format!(" stop={k}"))
                .unwrap_or_default();
            // The arguments that take their value from a column of a table.
            let refers = crate::marks::command_attributes(def.args.settings());
            let _ = writeln!(
                out,
                "{path} args={} reply={}{transmits}{stop}{refers} since={}",
                value(&args.join(";")),
                field_list(RecordDecl::fields(&def.reply)),
                since(&path)
            );
        }
    }
    out
}

/// A registry record, read from its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The key of the family.
    pub family: String,
    /// The version of the interface.
    pub version: Version,
    /// The items in the order of the file: the path and the `name=value` pairs.
    pub items: Vec<(String, BTreeMap<String, String>)>,
}

fn version(text: &str) -> Option<Version> {
    let (major, minor) = text.split_once('.')?;
    Some(Version {
        major: major.parse().ok()?,
        minor: minor.parse().ok()?,
    })
}

/// Reads a registry record.
///
/// # Errors
///
/// The number of the first line that is not a line of a record, with the reason.
pub fn parse(text: &str) -> Result<Record, String> {
    let mut record: Option<Record> = None;
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |why: &str| format!("line {}: {why}", n + 1);
        let mut words = line.split(' ');
        let path = words.next().unwrap_or_default();
        if path == "family" && record.is_none() {
            let key = words.next().ok_or_else(|| bad("no family key"))?;
            let v = words
                .next()
                .and_then(|w| w.strip_prefix("version="))
                .and_then(version)
                .ok_or_else(|| bad("no version"))?;
            record = Some(Record {
                family: key.to_owned(),
                version: v,
                items: Vec::new(),
            });
            continue;
        }
        let r = record
            .as_mut()
            .ok_or_else(|| bad("the family line is not first"))?;
        let mut pairs = BTreeMap::new();
        for w in words {
            let (name, v) = w
                .split_once('=')
                .ok_or_else(|| bad("a word is not name=value"))?;
            pairs.insert(name.to_owned(), v.to_owned());
        }
        if r.items.iter().any(|(p, _)| p == path) {
            return Err(bad("the path is there twice"));
        }
        r.items.push((path.to_owned(), pairs));
    }
    record.ok_or_else(|| "no family line".to_owned())
}

/// One difference between two records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    /// The path of the item.
    pub path: String,
    /// What changed, for people.
    pub what: String,
    /// An existing consumer does not survive the change.
    pub breaking: bool,
}

impl std::fmt::Display for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let class = if self.breaking {
            "breaking"
        } else {
            "compatible"
        };
        write!(f, "{class}: {} {}", self.path, self.what)
    }
}

/// True if `new` is the choice type `old` with more options after the old ones, or the flag set `old` with more flags
/// after the old ones.
fn options_added(old: &str, new: &str) -> bool {
    ["choice(", "flags("].iter().any(|kind| {
        let list = |t: &str| {
            t.strip_prefix(kind)
                .and_then(|t| t.strip_suffix(')'))
                .map(str::to_owned)
        };
        match (list(old), list(new)) {
            (Some(o), Some(n)) => n.starts_with(&o) && n[o.len()..].starts_with(','),
            _ => false,
        }
    })
}

/// True if the list `new` is the list `old` (entries separated by `;`) with more entries at its end. With `optional`, each
/// added entry is an optional argument (`?` after its type), and `old` must not be empty.
fn appended(old: &str, new: &str, optional: bool) -> bool {
    if old.is_empty() {
        return !optional && !new.is_empty();
    }
    match new
        .strip_prefix(old)
        .and_then(|rest| rest.strip_prefix(';'))
    {
        Some(rest) => !optional || rest.split(';').all(|a| a.ends_with('?')),
        None => false,
    }
}

/// The attributes of a registry line that hold a mark: of a table (the column that names a row, the default order, the
/// columns of a place, what the rows are, the columns of a message and of its delivery), of a setting (what it
/// chooses), of a field (its place among the key readings), of a setting or a command (the table and the column that
/// give values) and of the content line of a stack (the kind of content and the media).
const MARKS: [&str; 23] = [
    "label",
    "sort",
    "position",
    "latitude",
    "longitude",
    "altitude",
    "course",
    "speed",
    "kind",
    "audio",
    "pictures",
    "video",
    "text",
    "role",
    "sender",
    "addressee",
    "message",
    "time",
    "own",
    "delivery",
    "failure",
    "reading",
    "refers",
];

/// The differences from record `old` to record `new`. An added item is compatible. A removed item is breaking, so a
/// renamed item is one breaking removal and one addition. Another order of the fields of a shape is breaking. A changed
/// type, unit, row shape, argument list or version of appearance is breaking, and so is a field that became optional
/// or stopped being optional. More options at the end of a choice, more flags at the end of a flag set, another row limit of a table, optional arguments at
/// the end of the arguments of a command, fields at the end of the reply of a command, the content of a command
/// that starts to transmit, the stop command of a transmission that had none and a mark of a table or of a stack are compatible: a struct grows at its end, and a host built earlier leaves out what it does
/// not know.
pub fn compare(old: &Record, new: &Record) -> Vec<Change> {
    let mut out = Vec::new();
    let new_items: BTreeMap<&str, &BTreeMap<String, String>> =
        new.items.iter().map(|(p, a)| (p.as_str(), a)).collect();
    let old_paths: BTreeMap<&str, ()> = old.items.iter().map(|(p, _)| (p.as_str(), ())).collect();
    if old.family != new.family {
        out.push(Change {
            path: "family".into(),
            what: format!("renamed from {} to {}", old.family, new.family),
            breaking: true,
        });
    }
    for (path, was) in &old.items {
        let Some(now) = new_items.get(path.as_str()) else {
            out.push(Change {
                path: path.clone(),
                what: "removed".into(),
                breaking: true,
            });
            continue;
        };
        let names: std::collections::BTreeSet<&String> = was.keys().chain(now.keys()).collect();
        for name in names {
            let (a, b) = (was.get(name), now.get(name));
            if a == b {
                continue;
            }
            let text =
                |v: Option<&String>| v.map_or("nothing".to_owned(), |v| v.replace("%20", " "));
            // Who writes a table of a stack is not seen by a consumer. A mark of a table or of a stack is a statement
            // for a front end, and no layout depends on it.
            let compatible = name == "cap"
                || name == "writers"
                || MARKS.contains(&name.as_str())
                || (name == "type" && options_added(&text(a), &text(b)))
                || (name == "args" && appended(&text(a), &text(b), true))
                || (name == "reply" && appended(&text(a), &text(b), false))
                || (name == "transmits" && a.is_none())
                || (name == "stop" && a.is_none());
            out.push(Change {
                path: path.clone(),
                what: format!("{name} changed from {} to {}", text(a), text(b)),
                breaking: !compatible,
            });
        }
    }
    // The fields of a shape that both records hold keep their order: the order gives the layout of the struct.
    let fields = |r: &Record, other: &dyn Fn(&str) -> bool| -> Vec<(String, String)> {
        r.items
            .iter()
            .filter(|(p, _)| other(p))
            .filter_map(|(p, _)| {
                let (shape, field) = p.strip_prefix("shape.")?.split_once('.')?;
                Some((shape.to_owned(), field.to_owned()))
            })
            .collect()
    };
    let was = fields(old, &|p| new_items.contains_key(p));
    let now = fields(new, &|p| old_paths.contains_key(p));
    let mut moved: Vec<&str> = Vec::new();
    for ((shape, a), (_, b)) in was.iter().zip(&now) {
        if a != b && !moved.contains(&shape.as_str()) {
            moved.push(shape);
            out.push(Change {
                path: format!("shape.{shape}"),
                what: format!("the order of its fields changed at {a}"),
                breaking: true,
            });
        }
    }
    for (path, now) in &new.items {
        if !old_paths.contains_key(path.as_str()) {
            let since = now.get("since").map_or("", String::as_str);
            out.push(Change {
                path: path.clone(),
                what: format!("added in version {since}"),
                breaking: false,
            });
        }
    }
    out
}

/// What the differences between two records are, taken together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The records are equal.
    Same,
    /// Every difference is compatible and the minor version is raised. The list holds the differences.
    Compatible(Vec<Change>),
    /// A difference is breaking and the major version is raised. The list holds the differences.
    Breaking(Vec<Change>),
}

/// Makes sure that the version of `new` permits its differences from `old`: a breaking difference needs a higher major
/// version, and any other difference needs a higher version.
///
/// # Errors
///
/// The differences, one in a line, with the version to raise in the first line.
pub fn verdict(old: &Record, new: &Record) -> Result<Verdict, String> {
    let changes = compare(old, new);
    let list = |head: String| {
        let mut text = head;
        for c in &changes {
            let _ = write!(text, "\n  {c}");
        }
        text
    };
    if changes.is_empty() {
        return if old.version == new.version {
            Ok(Verdict::Same)
        } else {
            Err(format!(
                "the version changed from {} to {} and nothing else did",
                old.version, new.version
            ))
        };
    }
    if changes.iter().any(|c| c.breaking) {
        return if new.version.major > old.version.major {
            Ok(Verdict::Breaking(changes))
        } else {
            Err(list(format!(
                "the interface of {} has a breaking change at version {}: raise the major version to {}, or take the change back",
                new.family,
                new.version,
                old.version.major + 1
            )))
        };
    }
    let now = format!("added in version {}", new.version);
    if let Some(c) = changes
        .iter()
        .find(|c| c.what.starts_with("added") && c.what != now)
    {
        return Err(format!(
            "{} is new in version {}: list its path as added in that version",
            c.path, new.version
        ));
    }
    if new.version > old.version {
        Ok(Verdict::Compatible(changes))
    } else {
        Err(list(format!(
            "the interface of {} has additions at version {}: raise the minor version to {}.{}",
            new.family,
            new.version,
            old.version.major,
            old.version.minor + 1
        )))
    }
}

/// The check of the record file of `family` at `path`: the file is equal to the record of the description. If it
/// differs, the error holds the differences with their class. With `SDRKIT_WRITE_INTERFACE` set, the file is written
/// if [`verdict`] permits the change, or if there is no file yet.
///
/// # Errors
///
/// What to do, with the differences.
pub fn check(family: &Family, path: &std::path::Path) -> Result<(), String> {
    let text = render(family);
    let new = parse(&text).map_err(|e| format!("the record of the family {}: {e}", family.key))?;
    let write = crate::writes(path);
    let old = match std::fs::read_to_string(path) {
        Ok(t) => parse(&t).map_err(|e| format!("{}: {e}", path.display()))?,
        Err(_) if write => return crate::check_file(path, &text),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    match verdict(&old, &new)? {
        Verdict::Same => crate::check_file(path, &text),
        Verdict::Compatible(c) | Verdict::Breaking(c) if !write => {
            let lines: Vec<String> = c.iter().map(|c| format!("  {c}")).collect();
            Err(format!(
                "{} differs from the description. The version permits the change. Run the test with SDRKIT_WRITE_INTERFACE=1 to write it.\n{}",
                path.display(),
                lines.join("\n")
            ))
        }
        _ => crate::check_file(path, &text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "family air version=1.0\n\
        shape.Row since=1.0\n\
        shape.Row.icao type=text unit= optional=no since=1.0\n\
        shape.Row.alt_ft type=i64 unit=ft optional=yes since=1.0\n\
        shape.Row.fix type=choice(global,local) unit= optional=yes since=1.0\n\
        air since=1.0\n\
        air.table.aircraft row=Row cap=160 since=1.0\n";

    fn new(version: &str, edit: impl Fn(&str) -> String) -> Record {
        parse(&edit(OLD).replace("version=1.0", &format!("version={version}"))).unwrap()
    }

    #[test]
    fn an_addition_is_compatible_and_needs_the_minor_version() {
        let old = parse(OLD).unwrap();
        let add = |t: &str| format!("{t}shape.Row.squawk type=text unit= optional=yes since=1.1\n");
        let v = verdict(&old, &new("1.1", add)).unwrap();
        assert!(
            matches!(v, Verdict::Compatible(c) if c.len() == 1 && c[0].what == "added in version 1.1")
        );
        let e = verdict(&old, &new("1.0", add)).unwrap_err();
        assert!(e.contains("list its path as added"), "{e}");
        let same =
            |t: &str| format!("{t}shape.Row.squawk type=text unit= optional=yes since=1.0\n");
        let e = verdict(&old, &new("1.0", same)).unwrap_err();
        assert!(e.contains("raise the minor version to 1.1"), "{e}");
        // More options at the end of a choice, and another row limit.
        let more = |t: &str| {
            t.replace("choice(global,local)", "choice(global,local,none)")
                .replace("cap=160", "cap=300")
        };
        assert!(
            matches!(verdict(&old, &new("1.1", more)).unwrap(), Verdict::Compatible(c) if c.len() == 2)
        );
        // More flags at the end of a flag set. Another order, or a choice with the same keys, is another type.
        assert!(options_added("flags(crc,short)", "flags(crc,short,late)"));
        assert!(!options_added("flags(crc,short)", "flags(short,crc,late)"));
        assert!(!options_added("choice(crc,short)", "flags(crc,short,late)"));
    }

    #[test]
    fn a_removal_a_rename_and_a_change_of_type_or_unit_are_breaking() {
        let old = parse(OLD).unwrap();
        type Edit = fn(&str) -> String;
        let cases: [(&str, Edit); 5] = [
            ("shape.Row.alt_ft removed", |t| {
                t.replace(
                    "shape.Row.alt_ft type=i64 unit=ft optional=yes since=1.0\n",
                    "",
                )
            }),
            ("shape.Row.alt_ft removed", |t| {
                t.replace("shape.Row.alt_ft", "shape.Row.altitude_ft")
            }),
            ("shape.Row.alt_ft type changed from i64 to f64", |t| {
                t.replace("type=i64", "type=f64")
            }),
            ("shape.Row.alt_ft unit changed from ft to m", |t| {
                t.replace("unit=ft", "unit=m")
            }),
            ("shape.Row.fix type changed", |t| {
                t.replace("choice(global,local)", "choice(local,global)")
            }),
        ];
        for (what, edit) in cases {
            let e = verdict(&old, &new("1.1", edit)).unwrap_err();
            assert!(e.contains("raise the major version to 2"), "{e}");
            assert!(e.contains(&format!("breaking: {what}")), "{e}");
            assert!(matches!(
                verdict(&old, &new("2.0", edit)).unwrap(),
                Verdict::Breaking(_)
            ));
        }
    }

    #[test]
    fn another_order_of_the_fields_of_a_shape_is_breaking() {
        let old = parse(OLD).unwrap();
        let swap = |t: &str| {
            let mut lines: Vec<&str> = t.lines().collect();
            lines.swap(2, 3);
            lines.join("\n") + "\n"
        };
        let e = verdict(&old, &new("1.1", swap)).unwrap_err();
        assert!(
            e.contains("breaking: shape.Row the order of its fields changed at icao"),
            "{e}"
        );
        // A new field between two old ones moves no old field.
        let between = |t: &str| {
            t.replace(
                "shape.Row.alt_ft",
                "shape.Row.squawk type=text unit= optional=yes since=1.1\nshape.Row.alt_ft",
            )
        };
        assert!(
            matches!(verdict(&old, &new("1.1", between)).unwrap(), Verdict::Compatible(c) if c.len() == 1)
        );
    }

    #[test]
    fn appended_optional_arguments_reply_fields_and_a_transmit_content_are_compatible() {
        let old = parse(&format!(
            "{OLD}air.command.send args=text:text;to:text? reply= since=1.0\n\
             air.command.ask args=node:text reply=id:i64 since=1.0\n"
        ))
        .unwrap();
        let grow = |t: &str| {
            t.replace(
                "args=text:text;to:text? reply= since=1.0",
                "args=text:text;to:text?;hops:i64? reply=id:i64 transmits=text since=1.0",
            )
            .replace("reply=id:i64 since", "reply=id:i64;state:text since")
        };
        let text = |e: &dyn Fn(&str) -> String| {
            e(&format!(
                "{OLD}air.command.send args=text:text;to:text? reply= since=1.0\n\
                 air.command.ask args=node:text reply=id:i64 since=1.0\n"
            ))
        };
        let v = verdict(&old, &new("1.1", |_| text(&grow))).unwrap();
        assert!(
            matches!(&v, Verdict::Compatible(c) if c.len() == 4),
            "{v:?}"
        );
        // A required argument at the end, an argument in the middle, a reply field in the middle and another content
        // break a consumer.
        let breaking: [fn(&str) -> String; 4] = [
            |t| t.replace("to:text? reply= since", "to:text?;n:i64 reply= since"),
            |t| t.replace("text:text;to:text?", "text:text;n:i64?;to:text?"),
            |t| t.replace("reply=id:i64 since", "reply=state:text;id:i64 since"),
            |t| t.replace("reply= since", "reply= transmits=frame since"),
        ];
        for (i, edit) in breaking.into_iter().enumerate() {
            let changed = parse(&text(&edit).replace("version=1.0", "version=1.1")).unwrap();
            let before = if i == 3 {
                parse(&text(&|t: &str| {
                    t.replace("reply= since", "reply= transmits=text since")
                }))
                .unwrap()
            } else {
                old.clone()
            };
            let e = verdict(&before, &changed).unwrap_err();
            assert!(e.contains("raise the major version"), "{i}: {e}");
        }
    }

    #[test]
    fn equal_records_are_the_same() {
        let old = parse(OLD).unwrap();
        assert_eq!(verdict(&old, &old).unwrap(), Verdict::Same);
        assert!(parse("shape.Row since=1.0\n").is_err());
    }

    /// A stop command added to a transmission that had none is compatible; another or no stop command breaks a host
    /// that stops the transmission with it.
    #[test]
    fn a_new_stop_command_is_compatible_and_a_changed_one_breaks() {
        let line = |stop: &str| {
            format!(
                "{OLD}air.command.cq args= reply= transmits=text{stop} since=1.0\n\
                 air.command.halt args= reply= since=1.0\n"
            )
        };
        let none = parse(&line("")).unwrap();
        let halt = parse(&line(" stop=halt")).unwrap();
        let grown = parse(&line(" stop=halt").replace("version=1.0", "version=1.1")).unwrap();
        assert!(matches!(
            verdict(&none, &grown).unwrap(),
            Verdict::Compatible(c) if c.len() == 1
        ));
        for edit in ["", " stop=cq"] {
            let changed = parse(&line(edit).replace("version=1.0", "version=1.1")).unwrap();
            let e = verdict(&halt, &changed).unwrap_err();
            assert!(e.contains("raise the major version"), "{edit}: {e}");
        }
    }
}
