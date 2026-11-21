//! The declarations that a generic consumer reads from the stacks of the crate are the ones in
//! `tests/declarations.txt`. `SDRKIT_WRITE_DECLARATIONS=1` writes the file. Each item that the description keeps over
//! a change of frequency names a result of its stack.

use std::path::PathBuf;

#[test]
fn the_declarations_are_the_recorded_ones() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/declarations.txt");
    if let Err(e) =
        sdrkit_dev_interface::declarations::check(&sdrkit_stack_wlan::descriptors(), &path)
    {
        panic!("{e}");
    }
}

#[test]
fn each_kept_item_names_a_result_of_its_stack() {
    for s in &sdrkit_stack_wlan::typed::family().stacks {
        let kept = s.kept_keys().unwrap_or_else(|e| panic!("{}: {e}", s.id));
        assert!(!kept.is_empty(), "{}", s.id);
    }
}
