//! The stack families with an interface description. The registry record `registry/<family>.reg` is the one that the
//! description gives: a difference fails with its class, compatible or breaking, and `SDRKIT_WRITE_INTERFACE=1` writes
//! the record if the version of the family permits the difference. The source of the stack crate of a family
//! declares nothing by hand, and each described stack is in the stack list with the settings of its description.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

#[test]
fn the_registry_record_of_each_family_is_the_written_one() {
    let mut problems = Vec::new();
    let families = sdrkit_host_catalog::families();
    // A shared interface that a family uses has its own record.
    let shared = sdrkit_core::interface::shared_of(&families);
    for family in families.iter().chain(&shared) {
        let path = root().join(format!("registry/{}.reg", family.key));
        problems.extend(sdrkit_dev_interface::registry::check(family, &path).err());
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A described stack writes through its shapes only. A part of a crate that belongs to no described stack has not
/// moved yet: the test prints it with the number of its places and does not fail for it.
#[test]
fn a_described_stack_publishes_through_its_shapes_only() {
    let mut problems = Vec::new();
    for family in sdrkit_host_catalog::families() {
        let src = root().join(format!("crates/stacks/sdrkit-stack-{}/src", family.key));
        let described: Vec<&str> = family.stacks.iter().map(|s| s.id).collect();
        match sdrkit_dev_interface::sources::survey(&src, &described, &family.shared_sources) {
            Ok(found) => {
                let places: usize = found.not_moved.values().sum();
                if places > 0 {
                    let parts: Vec<String> = found
                        .not_moved
                        .iter()
                        .map(|(part, n)| format!("{part} {n}"))
                        .collect();
                    println!(
                        "{}: not moved, {places} places in {} parts: {}",
                        family.key,
                        parts.len(),
                        parts.join(", ")
                    );
                }
                problems.extend(found.problems);
            }
            Err(e) => problems.push(e),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The fields, columns, settings and arguments whose key names a unit that their kind does not hold: a report for a
/// later correction, family by family. It never fails.
#[test]
fn the_keys_with_a_unit_and_a_plain_number_are_listed() {
    for family in sdrkit_host_catalog::families() {
        let lines = sdrkit_dev_interface::units::report(family);
        if !lines.is_empty() {
            println!(
                "{}: {} keys with a unit and a plain number",
                family.key,
                lines.len()
            );
            for l in lines {
                println!("  {l}");
            }
        }
    }
}

/// The gate at the end of the move: no crate with a description writes outside its shapes anywhere.
#[test]
#[ignore = "switch on when every stack of every family has moved"]
fn a_family_publishes_through_its_shapes_only() {
    let mut problems = Vec::new();
    for family in sdrkit_host_catalog::families() {
        let src = root().join(format!("crates/stacks/sdrkit-stack-{}/src", family.key));
        problems.extend(sdrkit_dev_interface::sources::check(&src).err());
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_stack_is_in_at_most_one_family_and_every_described_stack_exists() {
    let stacks = sdrkit_host_catalog::all();
    for family in sdrkit_host_catalog::families() {
        for s in &family.stacks {
            let d = stacks.iter().find(|d| d.id == s.id);
            assert!(d.is_some(), "{}: no stack {}", family.key, s.id);
            // The settings that the stack list shows are those of the description.
            let listed = d.map(|d| (d.settings)());
            assert_eq!(
                listed.as_ref().map(|l| l.settings()),
                Some(s.schema().settings()),
                "{}",
                s.id
            );
            let owner = sdrkit_host_catalog::family_of(s.id).map(|f| f.key);
            assert_eq!(owner, Some(family.key), "{}", s.id);
        }
    }
}

/// Every stack of the stack list has a kind of content in the description of its family. A family states a kind or a
/// medium only for a stack that it holds.
#[test]
fn every_stack_states_its_kind_of_content() {
    let mut problems = Vec::new();
    for d in sdrkit_host_catalog::all() {
        let shape = sdrkit_host_catalog::family_of(d.id).and_then(|f| f.stack_shape(d.id));
        match shape {
            Some(s) if s.content.is_some() => {}
            Some(_) => problems.push(format!("{}: no kind of content", d.id)),
            None => problems.push(format!("{}: no description", d.id)),
        }
    }
    for family in sdrkit_host_catalog::families() {
        for (mark, ids) in &family.stack_marks {
            for id in ids.iter().filter(|id| family.stack_shape(id).is_none()) {
                problems.push(format!(
                    "{}: {mark:?} is stated for {id}, which is no stack of the family",
                    family.key
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
