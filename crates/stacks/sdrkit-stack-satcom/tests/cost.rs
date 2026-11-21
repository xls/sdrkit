//! The recorded cost of each stage (`cost.txt`) against the cost measured on the test signal, for the stacks of the
//! family that record one. `SDRKIT_WRITE_COST=1` in an optimised build writes the file.

use std::path::PathBuf;

/// The stacks that record their cost.
const STACKS: &[&str] = &["inmarsat"];

#[test]
fn the_recorded_cost_of_each_stage_is_the_measured_one_within_a_factor_of_two() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("cost.txt");
    for id in STACKS {
        let stacks = sdrkit_stack_satcom::descriptors();
        let stack = stacks.iter().find(|d| d.id == *id).expect("a stack of the family");
        if let Err(e) = sdrkit_graph::cost::check(stack, "-", &path, 2.0, |_| {}) {
            panic!("{e}");
        }
    }
}
