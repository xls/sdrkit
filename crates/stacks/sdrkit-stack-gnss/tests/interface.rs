//! The declarations that a generic consumer reads from the stacks of the crate are the ones in
//! `tests/declarations.txt`. `SDRKIT_WRITE_DECLARATIONS=1` writes the file.

use std::path::PathBuf;

#[test]
fn the_declarations_are_the_recorded_ones() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/declarations.txt");
    if let Err(e) =
        sdrkit_dev_interface::declarations::check(&sdrkit_stack_gnss::descriptors(), &path)
    {
        panic!("{e}");
    }
}
