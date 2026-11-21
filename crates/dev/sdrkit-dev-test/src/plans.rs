//! The band plans of the repository's `data/plans/` folder, loaded as a host loads them.

use sdrkit_data_plans::Plans;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Every `*.json` and `*.csv` file of the repository's `data/plans/` folder as (file name, text), in name order.
///
/// # Panics
///
/// If the folder or a file cannot be read: a fault of the repository.
pub fn plan_files() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data/plans");
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
        let path = entry
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        if name.ends_with(".json") || name.ends_with(".csv") {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            files.push((name, text));
        }
    }
    files.sort();
    files
}

/// The plans of [`plan_files`], read once.
///
/// # Panics
///
/// If a file is not a valid plan file: a fault of the repository.
pub fn plans() -> Arc<Plans> {
    static PLANS: OnceLock<Arc<Plans>> = OnceLock::new();
    Arc::clone(PLANS.get_or_init(|| {
        Arc::new(Plans::from_files(&plan_files()).unwrap_or_else(|e| panic!("data/plans: {e}")))
    }))
}
