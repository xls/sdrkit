//! Each plan file of the repository's data folder is valid on its own: an empty registry takes it as an import. The
//! file `countries.json` names the groups of every other file, so it is taken last, into a registry that holds them.

use sdrkit_data_plans::api::{import_as, is_plan_file, PlanFormat};
use sdrkit_data_plans::Plans;
use std::path::PathBuf;

/// The JSON plan files of `data/plans`, sorted by name, without `countries.json`.
fn files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans");
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .expect("the plans folder")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("json"))
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("countries.json"))
        .collect();
    out.sort();
    out
}

fn text(p: &PathBuf) -> String {
    std::fs::read_to_string(p).expect("a plan file")
}

#[test]
fn an_empty_registry_takes_each_plan_file_alone() {
    let mut problems = Vec::new();
    for p in files() {
        let t = text(&p);
        if !is_plan_file(&t) {
            continue;
        }
        let mut plans = Plans::new();
        if let Err(e) = import_as(&mut plans, &t, PlanFormat::Json, "", "") {
            problems.push(format!("{}: {e}", p.display()));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_country_file_follows_the_other_files() {
    let mut plans = Plans::new();
    for p in files() {
        let t = text(&p);
        if is_plan_file(&t) {
            import_as(&mut plans, &t, PlanFormat::Json, "", "").unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        }
    }
    let countries = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans/countries.json");
    import_as(&mut plans, &text(&countries), PlanFormat::Json, "", "").expect("the country file");
}
