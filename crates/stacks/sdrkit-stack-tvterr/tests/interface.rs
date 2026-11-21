//! The declarations that a generic consumer reads from the stacks of the crate are the ones in
//! `tests/declarations.txt`. `SDRKIT_WRITE_DECLARATIONS=1` writes the file.

use std::path::PathBuf;

#[test]
fn the_declarations_are_the_recorded_ones() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/declarations.txt");
    if let Err(e) =
        sdrkit_dev_interface::declarations::check(&sdrkit_stack_tvterr::descriptors(), &path)
    {
        panic!("{e}");
    }
}

#[test]
fn the_header_and_the_swift_file_of_the_family_can_be_written() {
    let family = sdrkit_stack_tvterr::typed::family();
    if let Err(e) = sdrkit_dev_interface::c_header::render(family) {
        panic!("{e}");
    }
    if let Err(e) = sdrkit_dev_interface::swift::render(family) {
        panic!("{e}");
    }
}
