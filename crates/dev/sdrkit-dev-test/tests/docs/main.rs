//! The pages under `docs/stacks` and `docs/drivers`, and the index.
//!
//! `pages_match_the_code` makes sure that every stack and every driver has a page and that the settings tables of each page
//! list the keys of the schema. `cargo test -p sdrkit-dev-test --test docs -- --ignored write_docs` writes the pages.
//!
//! A page has a part that the code writes, between the lines [`BEGIN`] and [`END`], and text that people write, above
//! or below that part. A write replaces the part between the two lines and keeps the rest.

mod drivers;
mod render;
mod source;
mod stacks;

use render::{code, table};
use sdrkit_core::net::FakeTransport;
use sdrkit_core::{Clock, MemStorage, StackDescriptor, Values};
use sdrkit_driver_core::SourceDriver;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The line before the part of a page that the code writes.
const BEGIN: &str = "<!-- generated: begin. The docs test writes the part down to the end line. Write your text below the end line. -->";
/// The line after the part of a page that the code writes.
const END: &str = "<!-- generated: end -->";

/// The headings of the sections that the code writes or wrote. In a page without the two lines, a section under
/// another heading is text that a person wrote.
const WRITTEN_HEADINGS: [&str; 22] = [
    "Tuning",
    "Submodes",
    "Settings",
    "Result fields",
    "Result tables",
    "Result series",
    "Result pictures",
    "Map points",
    "Events",
    "Commands",
    "Typed interface",
    "Try it",
    "Test recordings",
    "Address",
    "Radio settings",
    "Driver settings",
    "Transmit",
    "Streams it produces",
    "Commands of an open radio",
    "Commands of the driver",
    "Stacks",
    "Drivers",
];

/// The text of `old`, a page without the two lines, that a person wrote, in the order of the page. `generated` is the
/// part that the code writes now.
///
/// - A section under a heading that the code does not write is kept as it is.
/// - The text before the first section is kept under the heading `Description`, without the title and without the
///   lines that the code writes.
/// - Of a section under a heading that the code writes, the sentences and the code examples that the code does not
///   write are kept under the heading with `: notes` after it. A table with the header row of a table that the code
///   writes is not kept, nor the line that introduces it: the code writes that table. A table with another header
///   row is a table that a person wrote, and it is kept with the line that introduces it.
fn own_text(old: &str, generated: &str) -> String {
    let written: BTreeSet<&str> = generated
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect();
    // The page as sections: the heading (empty for the text before the first), and the lines.
    let mut sections: Vec<(&str, Vec<&str>)> = vec![("", Vec::new())];
    let mut in_fence = false;
    for line in old.lines() {
        if line.starts_with("```") {
            in_fence = !in_fence;
        }
        match line.strip_prefix("## ").filter(|_| !in_fence) {
            Some(h) => sections.push((h.trim(), Vec::new())),
            None => sections.last_mut().expect("one section").1.push(line),
        }
    }
    let mut out = String::new();
    for (heading, lines) in sections {
        let ours = heading.is_empty() || WRITTEN_HEADINGS.contains(&heading);
        if !ours {
            out.push_str(&format!("## {heading}\n{}\n", lines.join("\n")));
            continue;
        }
        let kept = unwritten(&lines, &written);
        if kept.iter().all(|l| l.is_empty()) {
            continue;
        }
        let title = if heading.is_empty() {
            "Description".to_string()
        } else {
            format!("{heading}: notes")
        };
        out.push_str(&format!(
            "## {title}\n\n{}\n\n",
            kept.join("\n").trim_matches('\n')
        ));
    }
    out
}

/// The lines of a section that the code does not write: no title, no line of `written`, and no table whose header
/// row is in `written`, with the line that introduces it. A code example stays whole unless the code writes every
/// line of it.
fn unwritten<'a>(lines: &[&'a str], written: &BTreeSet<&str>) -> Vec<&'a str> {
    let is_table = |l: &str| l.starts_with('|');
    let mut out: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("```") {
            let end = lines[i + 1..]
                .iter()
                .position(|l| l.starts_with("```"))
                .map_or(lines.len() - 1, |n| i + 1 + n);
            let example = &lines[i..=end];
            if !example
                .iter()
                .all(|l| l.is_empty() || written.contains(l.trim_end()))
            {
                out.extend_from_slice(example);
            }
            i = end + 1;
            continue;
        }
        if is_table(line) {
            // The whole table goes or stays by its header row.
            let end = lines[i..]
                .iter()
                .position(|l| !is_table(l))
                .map_or(lines.len(), |n| i + n);
            if !written.contains(line.trim_end()) {
                out.extend_from_slice(&lines[i..end]);
            }
            i = end;
            continue;
        }
        let next = lines[i + 1..].iter().find(|l| !l.is_empty());
        let leads_to_written_table = line.trim_end().ends_with(':')
            && next.is_some_and(|l| is_table(l) && written.contains(l.trim_end()));
        let drop =
            line.starts_with("# ") || leads_to_written_table || written.contains(line.trim_end());
        // One empty line between two kept paragraphs.
        if !drop && !(line.is_empty() && out.last().is_none_or(|l| l.is_empty())) {
            out.push(line);
        }
        i += 1;
    }
    out
}

/// The page with `generated` as the part that the code writes and the text of `old` that a person wrote. In a page
/// with the two lines, that text is everything above [`BEGIN`] and below [`END`]. In a page without them, it is what
/// [`own_text`] returns, and it goes below [`END`].
fn merge(old: Option<&str>, generated: &str) -> String {
    let (above, below) = match old {
        None => (String::new(), String::new()),
        Some(old) => match (old.find(BEGIN), old.find(END)) {
            (Some(b), Some(e)) if b < e => (old[..b].to_owned(), old[e + END.len()..].to_owned()),
            _ => (String::new(), own_text(old, generated)),
        },
    };
    let mut out = above;
    out.push_str(BEGIN);
    out.push('\n');
    out.push_str(generated.trim_end());
    out.push('\n');
    out.push_str(END);
    out.push('\n');
    let below = below.trim_matches('\n');
    if !below.is_empty() {
        out.push('\n');
        out.push_str(below);
        out.push('\n');
    }
    out
}

/// Writes the page at `path`: the part that the code writes becomes `generated`, the rest stays.
fn write_page(path: &Path, generated: &str) {
    let old = fs::read_to_string(path).ok();
    let text = merge(old.as_deref(), generated);
    fs::write(path, text).unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

fn docs_dir() -> PathBuf {
    source::root().join("docs")
}

fn make_drivers() -> Vec<Box<dyn SourceDriver>> {
    sdrkit_driver_registry::all(
        Arc::new(|_: &str, _: &Values, _| None),
        Arc::new(MemStorage::new()),
        Arc::new(FakeTransport::new()),
        Clock::new(|| 0.0),
        Arc::new(sdrkit_driver_usb::NoUsb),
    )
}

fn stack_page_path(id: &str) -> PathBuf {
    docs_dir().join("stacks").join(format!("{id}.md"))
}

fn driver_page_path(key: &str) -> PathBuf {
    docs_dir().join("drivers").join(format!("{key}.md"))
}

/// The table of contents.
fn index(stacks: &[&'static StackDescriptor], driver_keys: &[(String, String)]) -> String {
    let mut groups: Vec<&str> = Vec::new();
    for s in stacks {
        if !groups.contains(&s.group) {
            groups.push(s.group);
        }
    }
    let mut out = String::from(
        "# Stacks and drivers\n\nEach page lists the settings, results, events and commands of one stack or one driver. `sdrkit stacks` and `sdrkit drivers` print the lists from the program.\n\n## Stacks\n\n",
    );
    for g in groups {
        out.push_str(&format!("### {g}\n\n"));
        let rows: Vec<Vec<String>> = stacks
            .iter()
            .filter(|s| s.group == g)
            .map(|s| {
                vec![
                    format!("[{}](stacks/{}.md)", s.name, s.id),
                    s.blurb.to_string(),
                ]
            })
            .collect();
        out.push_str(&table(&["Stack", "What it does"], &rows));
        out.push('\n');
    }
    out.push_str("## Drivers\n\n");
    let rows: Vec<Vec<String>> = driver_keys
        .iter()
        .map(|(key, name)| {
            vec![
                format!("[{name}](drivers/{key}.md)"),
                code(key),
                drivers::connects(key).to_string(),
            ]
        })
        .collect();
    out.push_str(&table(&["Driver", "Key", "What it connects to"], &rows));
    out
}

/// Every page that the code produces: path and text.
fn all_pages() -> Vec<(PathBuf, String)> {
    let stacks = sdrkit_host_catalog::all();
    let ctx = stacks::Ctx::new();
    let mut pages: Vec<(PathBuf, String)> = stacks
        .iter()
        .map(|d| (stack_page_path(d.id), stacks::page(d, &ctx)))
        .collect();
    let mut keys = Vec::new();
    for d in make_drivers() {
        let key = d.descriptor().key;
        keys.push((key.to_string(), d.descriptor().name.to_string()));
        pages.push((driver_page_path(key), drivers::page(d)));
    }
    pages.push((docs_dir().join("index.md"), index(&stacks, &keys)));
    pages
}

/// Writes every page, and deletes a page of a stack or driver that is gone. With `SDRKIT_WRITE_ONLY` set, only the pages
/// whose paths hold one of its parts (comma between them) are written or deleted.
#[test]
#[ignore = "writes the pages under docs"]
fn write_docs() {
    let only = std::env::var("SDRKIT_WRITE_ONLY").ok();
    let chosen = |path: &Path| {
        let p = path.to_string_lossy();
        only.as_deref().is_none_or(|o| {
            o.split(',')
                .map(str::trim)
                .any(|part| !part.is_empty() && p.contains(part))
        })
    };
    let pages: Vec<_> = all_pages().into_iter().filter(|(p, _)| chosen(p)).collect();
    for sub in ["stacks", "drivers"] {
        let dir = docs_dir().join(sub);
        fs::create_dir_all(&dir).expect("the folder can be made");
        for entry in fs::read_dir(&dir)
            .expect("the folder can be read")
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md")
                && chosen(&path)
                && !pages.iter().any(|(p, _)| *p == path)
            {
                fs::remove_file(&path).expect("the old page can be removed");
            }
        }
    }
    for (path, text) in pages {
        write_page(&path, &text);
    }
}

/// With `SDRKIT_WRITE_INTERFACE` set, writes the pages of the stacks that are in a family with an interface
/// description, and of the stacks that the environment variable `SDRKIT_DOCS_STACKS` names (ids with a comma between
/// them), and no other page. `SDRKIT_WRITE_ONLY` limits the pages that it writes ([`sdrkit_dev_interface::writes`]). Without the variable it does nothing: the other tests compare the pages.
#[test]
fn the_pages_of_the_described_families_are_written() {
    if std::env::var_os("SDRKIT_WRITE_INTERFACE").is_none() {
        return;
    }
    let ctx = stacks::Ctx::new();
    let named = std::env::var("SDRKIT_DOCS_STACKS").unwrap_or_default();
    // A stack whose receiver does not start does not stop the pages of the other stacks.
    let mut problems = Vec::new();
    for d in sdrkit_host_catalog::all() {
        if sdrkit_host_catalog::family_of(d.id).is_some() || named.split(',').any(|id| id == d.id) {
            match ctx.starts(d) {
                Ok(()) if !sdrkit_dev_interface::writes(&stack_page_path(d.id)) => {}
                Ok(()) => write_page(&stack_page_path(d.id), &stacks::page(d, &ctx)),
                Err(e) => problems.push(e),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_write_keeps_the_text_that_a_person_wrote() {
    let old = "# A\n\nBlurb.\n\nIntro by hand.\n\n## Settings\n\nThe thresholds:\n\n| key | value |\n|---|---|\n| a | 1 |\n\nGrouped by hand:\n\n| group | keys |\n|---|---|\n| g | a |\n\nA note by hand.\n\n```sh\nrun by hand\n```\n\n## How it works\n\nBy hand.\n";
    let first = merge(
        Some(old),
        "# A\n\nBlurb.\n\n## Settings\n\n| key | value |\n|---|---|\n| a | 2 |\n",
    );
    // The table that the code writes replaces the one of the page. The table that a person wrote stays, with the line
    // that introduces it.
    assert!(
        first.contains("| a | 2 |")
            && !first.contains("| a | 1 |")
            && !first.contains("The thresholds:")
    );
    assert!(first.contains("Grouped by hand:\n\n| group | keys |\n|---|---|\n| g | a |\n"));
    assert_eq!(first.matches("Blurb.").count(), 1);
    assert!(first.contains("## Description\n\nIntro by hand.\n"));
    assert!(first.contains("A note by hand.\n\n```sh\nrun by hand\n```\n"));
    assert!(first.ends_with("## How it works\n\nBy hand.\n"));
    let edited = format!("Above.\n{first}\nBelow.\n");
    let second = merge(Some(&edited), "# A\n\n## Settings\n\nnewer table\n");
    assert!(second.starts_with("Above.\n") && second.ends_with("By hand.\n\nBelow.\n"));
    assert!(second.contains("newer table") && !second.contains("| a | 2 |"));
    assert_eq!(second.matches(BEGIN).count(), 1);
}

/// The keys in the first column of the table that follows the heading line `heading`. `None` if the page has no such heading
/// or no table under it.
fn table_keys(page: &str, heading: &str) -> Option<Vec<String>> {
    let mut lines = page.lines();
    lines.by_ref().find(|l| *l == heading)?;
    // Every table of the section: the settings, then the advanced settings.
    let mut keys = Vec::new();
    let mut in_table = 0;
    for l in lines {
        if l.starts_with("## ") {
            break;
        }
        if !l.starts_with('|') {
            // After the settings, only the tables of the advanced settings and of the generator settings belong to them.
            if !keys.is_empty()
                && !l.is_empty()
                && !l.starts_with("Advanced settings.")
                && l != render::GENERATOR_SETTINGS_LEAD
            {
                break;
            }
            in_table = 0;
            continue;
        }
        in_table += 1;
        // The first two lines of a table are its header and its rule.
        if in_table > 2 {
            let first = l.trim_start_matches('|').split('|').next().unwrap_or("");
            keys.push(first.trim().trim_matches('`').to_string());
        }
    }
    (!keys.is_empty()).then_some(keys)
}

/// Names the keys that differ between what the schema has and what the page lists.
fn compare(what: &str, expected: &[String], found: &[String], problems: &mut Vec<String>) {
    let want: BTreeSet<&String> = expected.iter().collect();
    let have: BTreeSet<&String> = found.iter().collect();
    let missing: Vec<&&String> = want.difference(&have).collect();
    let extra: Vec<&&String> = have.difference(&want).collect();
    if !missing.is_empty() {
        problems.push(format!("{what}: the table lacks the keys {missing:?}"));
    }
    if !extra.is_empty() {
        problems.push(format!(
            "{what}: the table has keys that are not settings: {extra:?}"
        ));
    }
    if found.len() != have.len() {
        problems.push(format!("{what}: the table lists a key twice"));
    }
}

fn read(path: &Path, problems: &mut Vec<String>) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(t) => Some(t),
        Err(_) => {
            problems.push(format!("missing page {}", path.display()));
            None
        }
    }
}

/// Pages in `sub` that no stack or driver has.
fn stale_pages(sub: &str, names: &BTreeSet<String>, problems: &mut Vec<String>) {
    let Ok(read) = fs::read_dir(docs_dir().join(sub)) else {
        return;
    };
    for e in read.flatten() {
        let path = e.path();
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if path.extension().is_some_and(|x| x == "md") && !names.contains(stem) {
            problems.push(format!("page {} belongs to no {sub} entry", path.display()));
        }
    }
}

#[test]
fn pages_match_the_code() {
    let mut problems = Vec::new();
    let stacks = sdrkit_host_catalog::all();
    let index_text = read(&docs_dir().join("index.md"), &mut problems).unwrap_or_default();

    let mut ids = BTreeSet::new();
    for d in &stacks {
        ids.insert(d.id.to_string());
        if !index_text.contains(&format!("(stacks/{}.md)", d.id)) {
            problems.push(format!("the index does not list the stack {}", d.id));
        }
        let Some(page) = read(&stack_page_path(d.id), &mut problems) else {
            continue;
        };
        let expected: Vec<String> = (d.settings)()
            .settings()
            .iter()
            .map(|s| s.field.key.to_string())
            .collect();
        let found = table_keys(&page, "## Settings").unwrap_or_default();
        compare(&format!("stack {}", d.id), &expected, &found, &mut problems);
    }
    stale_pages("stacks", &ids, &mut problems);

    let mut keys = BTreeSet::new();
    for d in make_drivers() {
        let desc = d.descriptor();
        keys.insert(desc.key.to_string());
        if !index_text.contains(&format!("(drivers/{}.md)", desc.key)) {
            problems.push(format!("the index does not list the driver {}", desc.key));
        }
        let Some(page) = read(&driver_page_path(desc.key), &mut problems) else {
            continue;
        };
        let mut found = table_keys(&page, "## Radio settings").unwrap_or_default();
        found.extend(table_keys(&page, "## Driver settings").unwrap_or_default());
        let what = format!("driver {}", desc.key);
        compare(&what, &drivers::schema_keys(&*d), &found, &mut problems);
        let transmit = table_keys(&page, "## Transmit");
        if desc.accepts.is_empty() {
            if transmit.is_some() {
                problems.push(format!(
                    "{what}: the page has transmit settings, the driver cannot transmit"
                ));
            }
        } else {
            let expected: Vec<String> = drivers::transmit_settings(desc.key)
                .iter()
                .map(|s| s.field.key.to_string())
                .collect();
            compare(
                &format!("{what} (transmit)"),
                &expected,
                &transmit.unwrap_or_default(),
                &mut problems,
            );
        }
    }
    stale_pages("drivers", &keys, &mut problems);

    let listed: BTreeSet<String> = drivers::keys().into_iter().map(String::from).collect();
    if listed != keys {
        problems.push(format!(
            "the address list and the registry differ: {listed:?} against {keys:?}"
        ));
    }

    assert!(
        problems.is_empty(),
        "run `cargo test -p sdrkit-dev-test --test docs -- --ignored write_docs` to write the pages.\n{}",
        problems.join("\n")
    );
}
