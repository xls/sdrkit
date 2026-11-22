//! The declarations that a generic consumer reads from a stack, as text: settings, result fields, tables, series,
//! rasters, map points, events and commands, with their keys, labels, units, kinds, choice lists and order. A stack
//! crate keeps the text in `tests/declarations.txt`, so a change of a declaration shows as a difference.

use sdrkit_core::{Receiver, StackConfig, StackDescriptor, Values};
use std::fmt::Write;
use std::path::Path;

fn dump(d: &StackDescriptor, out: &mut String) {
    let schema = (d.settings)();
    let rx: Box<dyn Receiver> = (d.make_receiver)(&StackConfig::new(Values::defaults(&schema)))
        .expect("the stack starts with its defaults");
    let _ = writeln!(out, "stack {}", d.id);
    for s in schema.settings() {
        let _ = writeln!(out, "setting {s:?}");
    }
    let decl = rx.results_decl();
    for f in decl.fields().fields() {
        let _ = writeln!(out, "field {f:?}");
    }
    for t in decl.tables() {
        let _ = writeln!(out, "table {:?} {:?} cap {}", t.key, t.label, t.cap);
        if !t.marks.is_empty() {
            let _ = writeln!(out, "  marks {:?}", t.marks);
        }
        for c in t.columns.fields() {
            let _ = writeln!(out, "  column {c:?}");
        }
    }
    for s in decl.series_decls() {
        let _ = writeln!(out, "series {s:?}");
    }
    for r in decl.rasters() {
        let _ = writeln!(out, "raster {r:?}");
    }
    let _ = writeln!(out, "points {:?}", decl.points_label());
    for e in decl.events() {
        let _ = writeln!(out, "event {:?} {:?}", e.kind, e.label);
        for f in e.fields.fields() {
            let _ = writeln!(out, "  field {f:?}");
        }
    }
    for c in rx.commands().defs() {
        let _ = write!(out, "command {:?} {:?} {:?}", c.key, c.label, c.help);
        if let Some(t) = c.transmits {
            let _ = write!(out, " transmits {:?}", sdrkit_core::Choice::key(t));
        }
        if let Some(k) = &c.stopped_by {
            let _ = write!(out, " stopped_by {k:?}");
        }
        let _ = writeln!(out);
        for a in c.args.settings() {
            let _ = writeln!(out, "  argument {a:?}");
        }
        for f in c.reply.fields() {
            let _ = writeln!(out, "  reply {f:?}");
        }
    }
}

/// The declarations of `stacks`, each made with its default settings.
///
/// # Panics
///
/// If a stack does not start with its defaults.
pub fn render(stacks: &[&StackDescriptor]) -> String {
    let mut out = String::new();
    for d in stacks {
        dump(d, &mut out);
    }
    out
}

/// Compares the declarations of `stacks` with the text in the file at `path`. With the environment variable
/// `SDRKIT_WRITE_DECLARATIONS` set, it writes the file.
///
/// # Errors
///
/// The number of the first line that differs, with both lines, or why the file cannot be read or written.
pub fn check(stacks: &[&StackDescriptor], path: &Path) -> Result<(), String> {
    let now = render(stacks);
    if std::env::var_os("SDRKIT_WRITE_DECLARATIONS").is_some() {
        return std::fs::write(path, &now).map_err(|e| format!("{}: {e}", path.display()));
    }
    let recorded = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    for (n, (a, b)) in recorded.lines().zip(now.lines()).enumerate() {
        if a != b {
            return Err(format!("line {}:\n  recorded: {a}\n  now:      {b}", n + 1));
        }
    }
    if recorded.lines().count() != now.lines().count() {
        return Err(format!(
            "{} lines recorded, {} now",
            recorded.lines().count(),
            now.lines().count()
        ));
    }
    Ok(())
}
