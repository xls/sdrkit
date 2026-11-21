//! Finds the stack crates that have an interface description (`src/typed.rs`) and sets the cfg `typed_<family>` for
//! each, so the family list of the catalog needs no line for a new description.

use std::path::Path;

fn main() {
    let stacks = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../stacks");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", stacks.display());
    let Ok(entries) = std::fs::read_dir(&stacks) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(family) = name.to_str().and_then(|n| n.strip_prefix("sdrkit-stack-")) else {
            continue;
        };
        println!("cargo:rustc-check-cfg=cfg(typed_{family})");
        let typed = entry.path().join("src/typed.rs");
        println!("cargo:rerun-if-changed={}", typed.display());
        if typed.is_file() {
            println!("cargo:rustc-cfg=typed_{family}");
        }
    }
}
