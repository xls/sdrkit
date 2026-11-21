//! What the pages read from the source tree: the module header that describes a stack and the recordings that exercise it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The root of the workspace.
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n != "tests") {
                rust_files(&p, out);
            }
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Where a stack is declared.
pub struct Home {
    /// The crate family: `fm` for `sdrkit-stack-fm`.
    pub family: String,
    /// The source file that holds the descriptor.
    pub file: PathBuf,
    /// How many stack ids that file declares.
    pub ids_in_file: usize,
}

/// The home of each stack id, found by reading the descriptors in `crates/stacks`.
pub fn homes() -> BTreeMap<String, Home> {
    let mut map = BTreeMap::new();
    let Ok(crates) = fs::read_dir(root().join("crates/stacks")) else {
        return map;
    };
    let mut crates: Vec<PathBuf> = crates.filter_map(|e| e.ok().map(|e| e.path())).collect();
    crates.sort();
    for c in crates {
        let family = c
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_prefix("sdrkit-stack-"))
            .unwrap_or_default()
            .to_string();
        let mut files = Vec::new();
        rust_files(&c.join("src"), &mut files);
        for f in files {
            let Ok(text) = fs::read_to_string(&f) else {
                continue;
            };
            if !text.contains("StackDescriptor {") {
                continue;
            }
            let ids: Vec<String> = text
                .lines()
                .filter_map(|l| {
                    let l = l.trim();
                    let rest = l.strip_prefix("id: \"")?;
                    Some(rest.split('"').next()?.to_string())
                })
                .collect();
            for id in &ids {
                map.entry(id.clone()).or_insert(Home {
                    family: family.clone(),
                    file: f.clone(),
                    ids_in_file: ids.len(),
                });
            }
        }
    }
    map
}

/// The module header of a file: the text of its leading `//!` lines.
fn header_lines(file: &Path) -> Vec<String> {
    let Ok(text) = fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .take_while(|l| l.trim_start().starts_with("//!"))
        .map(|l| l.trim_start().trim_start_matches("//!").trim().to_string())
        .collect()
}

/// Removes `[`x`]` links and the brackets that list only code names.
fn clean(text: &str) -> String {
    let text = text.replace("[`", "`").replace("`]", "`");
    let mut out = String::new();
    let mut rest = text.as_str();
    while let Some(open) = rest.find('(') {
        out.push_str(&rest[..open]);
        let tail = &rest[open..];
        match tail.find(')') {
            Some(close) if only_names(&tail[1..close]) => {
                rest = &tail[close + 1..];
                if out.ends_with(' ') && rest.starts_with([' ', '.']) {
                    out.pop();
                }
            }
            Some(close) => {
                out.push_str(&tail[..=close]);
                rest = &tail[close + 1..];
            }
            None => {
                out.push_str(tail);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// True for text such as "`a`, `b` and `c`" that holds only code names.
fn only_names(text: &str) -> bool {
    let mut in_code = false;
    let mut any = false;
    for c in text.chars() {
        if c == '`' {
            in_code = !in_code;
            any = true;
        } else if !in_code && !(c == ',' || c == ' ') {
            return false;
        }
    }
    any
}

/// What the header says about a stack, in at most three sentences, and the standard line if it has one. Empty if the file
/// declares several stacks, because its header then describes all of them.
pub fn describe(home: &Home) -> (String, String) {
    if home.ids_in_file != 1 {
        return (String::new(), String::new());
    }
    let lines = header_lines(&home.file);
    let standard = lines
        .iter()
        .find_map(|l| l.strip_prefix("Standard:"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let body: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .filter(|l| !l.starts_with("Standard:") && !l.is_empty())
        .collect();
    let text = clean(&body.join(" "));
    let mut sentences = Vec::new();
    for s in text.split_inclusive(". ") {
        let s = s.trim();
        if s.contains("DESCRIPTOR") || s.contains("engine knows") || s.is_empty() {
            continue;
        }
        sentences.push(s.to_string());
        if sentences.len() == 3 {
            break;
        }
    }
    (sentences.join(" "), standard)
}

/// The recordings in `tests/vectors` that exercise a stack: those of its family whose name has the stack id or a submode id
/// as one of its parts. Each is the repository path of the `.sigmf-meta` file.
pub fn vectors(family: &str, names: &[&str]) -> Vec<String> {
    let dir = root().join("tests/vectors");
    let mut found = Vec::new();
    for sub in [family.to_string(), format!("real/{family}")] {
        let Ok(read) = fs::read_dir(dir.join(&sub)) else {
            continue;
        };
        let mut files: Vec<PathBuf> = read.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.sort();
        for f in files {
            let Some(name) = f.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(stem) = name.strip_suffix(".sigmf-meta") else {
                continue;
            };
            if stem.split('_').any(|part| names.contains(&part)) {
                found.push(format!("tests/vectors/{sub}/{name}"));
            }
        }
    }
    found
}
