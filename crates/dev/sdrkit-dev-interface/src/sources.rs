//! The check of the source of a stack crate that has an interface description: a described stack declares no field,
//! table or event type outside its shapes, so every value that it publishes goes through a struct of the description.
//! [`survey`] is the check while some stacks of the crate have not moved, and [`check`] the check at the end.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What a crate with an interface description does not write: a declaration by hand, a write of one cell, and the
/// accessor that the derive uses.
const FORBIDDEN: [&str; 12] = [
    "row_mut(",
    "write_standard(",
    "impl Sink",
    "(&keys::",
    "(&columns::",
    "Field::new(",
    "TableDef {",
    "EventDef::new(",
    "impl Cells",
    "Object for ",
    "__field(",
    "publish::rows",
];

/// One place where a source file writes outside the shapes: the file, the line and what it holds.
struct Place {
    file: PathBuf,
    line: usize,
    what: &'static str,
}

impl Place {
    fn text(&self) -> String {
        format!("{}:{}: {}", self.file.display(), self.line, self.what)
    }
}

/// True if `code` holds `word` as a whole name: the character before it is not part of an identifier. A name that
/// stands as the return type of a function before its body (`-> TableDef {`, `-> &'static TableDef {`) is a type,
/// not a declaration, and does not count.
fn has_word(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        let before = &code[..at];
        let in_name = before
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_');
        !in_name && !returns(before)
    })
}

/// True if `before`, the code before a name, ends in the arrow of a return type, with or without a reference.
fn returns(before: &str) -> bool {
    let t = before.trim_end();
    let t = t.strip_suffix("'static").map_or(t, str::trim_end);
    let t = t.strip_suffix('&').map_or(t, str::trim_end);
    t.ends_with("->")
}

fn scan(dir: &Path, found: &mut Vec<Place>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            scan(&path, found)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path)?;
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if let Some(w) = FORBIDDEN.iter().find(|w| has_word(code, w)) {
                    found.push(Place {
                        file: path.clone(),
                        line: n + 1,
                        what: w.trim_end_matches('('),
                    });
                }
            }
        }
    }
    Ok(())
}

/// Reads every `.rs` file under `src_dir` and makes sure that none declares a field, a table or an event type by hand
/// or writes one cell. This is the check of a crate whose stacks have all moved to the description.
///
/// # Errors
///
/// One line for each place, or why a file cannot be read.
pub fn check(src_dir: &Path) -> Result<(), String> {
    let mut found = Vec::new();
    scan(src_dir, &mut found).map_err(|e| format!("{}: {e}", src_dir.display()))?;
    if found.is_empty() {
        Ok(())
    } else {
        let lines: Vec<String> = found.iter().map(Place::text).collect();
        Err(format!(
            "a stack with an interface description publishes through its shapes only:\n{}",
            lines.join("\n")
        ))
    }
}

/// What [`survey`] found in a crate of which some stacks have moved to the description.
#[derive(Debug, Default)]
pub struct Survey {
    /// The places in the source of a described stack that write outside its shapes, one line each.
    pub problems: Vec<String>,
    /// The parts of the crate that have not moved, each with the number of its places: the first folder or file under
    /// `src` of a source file that belongs to no described stack.
    pub not_moved: BTreeMap<String, usize>,
}

/// Returns true if the source file `rel`, a path under `src`, belongs to the stack `id`: a folder or the file of the
/// path has the name of the stack, with or without `_`.
fn belongs(rel: &Path, id: &str) -> bool {
    let plain = |s: &str| s.replace(['_', '-'], "").to_ascii_lowercase();
    let id = plain(id);
    rel.components().any(|c| {
        let name = c.as_os_str().to_string_lossy();
        plain(name.strip_suffix(".rs").unwrap_or(&name)) == id
    })
}

/// The stacks that the source file `rel` belongs to by an entry of `shared`: the first entry whose name a folder or the
/// file of the path has.
fn sharers<'a>(rel: &Path, shared: &'a [(&str, &[&str])]) -> Option<&'a [&'a str]> {
    shared
        .iter()
        .find(|(name, _)| belongs(rel, name))
        .map(|(_, stacks)| *stacks)
}

/// Reads every `.rs` file under `src_dir` of a crate whose description lists the stacks `described`. `shared` names
/// the files and folders that several stacks use, each with the ids of these stacks
/// ([`Family::shared_source`](sdrkit_core::interface::Family::shared_source)). A place that writes outside the shapes
/// is a problem if its file belongs to a described stack, or to shared code of which every stack is described. If
/// not, it counts as not moved.
///
/// # Errors
///
/// Why a file cannot be read.
pub fn survey(
    src_dir: &Path,
    described: &[&str],
    shared: &[(&str, &[&str])],
) -> Result<Survey, String> {
    let mut found = Vec::new();
    scan(src_dir, &mut found).map_err(|e| format!("{}: {e}", src_dir.display()))?;
    let mut out = Survey::default();
    for place in found {
        let rel = place.file.strip_prefix(src_dir).unwrap_or(&place.file);
        let moved = match sharers(rel, shared) {
            Some(stacks) => stacks.iter().all(|id| described.contains(id)),
            None => described.iter().any(|id| belongs(rel, id)),
        };
        if moved {
            out.problems.push(place.text());
        } else {
            let part = rel
                .components()
                .next()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .unwrap_or_default();
            *out.not_moved.entry(part).or_default() += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_return_type_is_not_a_declaration_by_hand() {
        assert!(!has_word(
            "    fn table() -> &'static TableDef {",
            "TableDef {"
        ));
        assert!(!has_word(
            "    fn table(&self) -> &TableDef {",
            "TableDef {"
        ));
        assert!(!has_word("fn table() -> TableDef {", "TableDef {"));
        assert!(has_word(
            "pub static T: TableDef = TableDef {",
            "TableDef {"
        ));
        assert!(has_word("static T: &TableDef = &TableDef {", "TableDef {"));
        assert!(has_word("    let t = TableDef {", "TableDef {"));
        assert!(!has_word("let t = MyTableDef {", "TableDef {"));
    }

    #[test]
    fn a_file_belongs_to_the_stack_of_its_folder_or_its_name() {
        assert!(belongs(
            Path::new("modes/inmarsat_c/display.rs"),
            "inmarsatc"
        ));
        assert!(belongs(Path::new("ltr.rs"), "ltr"));
        assert!(!belongs(Path::new("ltr_net/mod.rs"), "ltr"));
        assert!(!belongs(Path::new("common/tables.rs"), "ltr"));
    }

    #[test]
    fn a_shared_file_belongs_to_the_stacks_that_the_description_names() {
        let shared: [(&str, &[&str]); 1] = [("teleprinter", &["rtty", "sitor"])];
        assert_eq!(
            sharers(Path::new("teleprinter.rs"), &shared),
            Some(&["rtty", "sitor"][..])
        );
        assert_eq!(
            sharers(Path::new("teleprinter/code.rs"), &shared),
            Some(&["rtty", "sitor"][..])
        );
        assert_eq!(sharers(Path::new("rtty.rs"), &shared), None);
    }
}
