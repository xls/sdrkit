//! Every stack whose content is readings states which of its result fields a person looks at first, so that a front
//! end shows them large without the keys of the fields.

use sdrkit_core::{Choice, ContentKind, FieldDecl};

/// The stacks of the three kinds without a key reading, each with the reason.
const WITHOUT: &[(&str, &str)] = &[];

#[test]
fn a_stack_of_readings_states_its_key_readings() {
    let kinds = [
        ContentKind::Measurement,
        ContentKind::Science,
        ContentKind::Navigation,
    ];
    let mut problems = Vec::new();
    let mut seen = 0;
    for d in sdrkit_host_catalog::all() {
        let Some(shape) = sdrkit_host_catalog::family_of(d.id).and_then(|f| f.stack_shape(d.id))
        else {
            continue;
        };
        let readings = sdrkit_dev_interface::marks::key_readings(shape);
        if readings.len() > usize::from(FieldDecl::MAX_READINGS) {
            problems.push(format!(
                "{}: {} key readings, at most {}",
                d.id,
                readings.len(),
                FieldDecl::MAX_READINGS
            ));
        }
        // The orders count from 1 without a gap, so that a front end shows them in one order.
        for (n, (order, key)) in readings.iter().enumerate() {
            if usize::from(*order) != n + 1 {
                problems.push(format!(
                    "{}: the key reading {key} has the order {order}, and it is number {}",
                    d.id,
                    n + 1
                ));
                break;
            }
        }
        let Some(kind) = shape.content.filter(|k| kinds.contains(k)) else {
            continue;
        };
        seen += 1;
        let excused = WITHOUT.iter().find(|(id, _)| *id == d.id);
        match (readings.is_empty(), excused) {
            (true, None) => problems.push(format!(
                "{} ({}): no result field is a key reading",
                d.id,
                kind.key()
            )),
            (false, Some((_, why))) => problems.push(format!(
                "{}: the test excuses it ({why}), and it has a key reading",
                d.id
            )),
            _ => {}
        }
    }
    assert!(seen >= 60, "{seen} stacks of the three kinds");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
