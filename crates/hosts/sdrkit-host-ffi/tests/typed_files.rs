//! The files written from the interface description of each stack family of the catalog and of the session, the radio
//! and the engine: the C header `include/sdrkit/<family>.h` and the Swift file `swift/<Family>.swift` are equal to what
//! the description gives, and `swift/SDRKit.swift` is the file that they share. `include/sdrkit/init.h` holds the
//! initialisers of the sized structs of `include/sdrkit.h`. `SDRKIT_WRITE_INTERFACE=1` writes them.

use sdrkit_dev_interface::{c_header, check_file, swift};
use std::path::PathBuf;

#[test]
fn the_family_headers_and_swift_files_are_written_from_the_descriptions() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut problems = Vec::new();
    let base = std::fs::read_to_string(dir.join("include/sdrkit.h")).expect("the base header");
    let init = sdrkit_dev_interface::base::render_init(&base);
    problems.extend(check_file(&dir.join("include/sdrkit/init.h"), &init).err());
    problems.extend(check_file(&dir.join("swift/SDRKit.swift"), &swift::render_base(&base)).err());
    for family in sdrkit::families() {
        // The registry records of the stack families are checked with the catalog. Those of the objects are checked here.
        if family.object {
            let record = dir.join(format!("../../../registry/{}.reg", family.key));
            problems.extend(sdrkit_dev_interface::registry::check(family, &record).err());
        }
        // A family whose files cannot be written does not stop the files of the other families.
        match c_header::render(family) {
            Ok(header) => {
                let path = dir.join(format!("include/sdrkit/{}.h", family.key));
                problems.extend(check_file(&path, &header).err());
            }
            Err(e) => problems.push(format!("the header of the family {}: {e}", family.key)),
        }
        match swift::render(family) {
            Ok(text) => {
                let mut name = family.key.to_owned();
                name[..1].make_ascii_uppercase();
                let path = dir.join(format!("swift/{name}.swift"));
                problems.extend(check_file(&path, &text).err());
            }
            Err(e) => problems.push(format!("the Swift file of the family {}: {e}", family.key)),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
