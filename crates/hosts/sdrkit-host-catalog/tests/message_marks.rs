//! Every stack whose content is messages or calls states which of its tables holds them, so that a front end finds the
//! message log, the call log, the nodes and the groups without the keys of the tables.

use sdrkit_core::{ContentKind, TableMarks, TableRole};

/// The stacks of the five kinds without a table of messages, calls, nodes or groups, each with the reason.
const WITHOUT: &[(&str, &str)] = &[
    ("ldacs", "it detects the OFDM signal and reads no message"),
    (
        "aeromacs",
        "it detects the OFDM signal and reads no message",
    ),
];

#[test]
fn a_stack_of_messages_or_calls_states_the_role_of_its_tables() {
    let kinds = [
        ContentKind::Mesh,
        ContentKind::Paging,
        ContentKind::Datalink,
        ContentKind::TextChat,
        ContentKind::Voice,
    ];
    let mut problems = Vec::new();
    let mut seen = 0;
    for d in sdrkit_host_catalog::all() {
        let Some(shape) = sdrkit_host_catalog::family_of(d.id).and_then(|f| f.stack_shape(d.id))
        else {
            continue;
        };
        let Some(kind) = shape.content else {
            continue;
        };
        let marks: Vec<(&str, TableMarks)> = shape
            .tables
            .iter()
            .map(|t| {
                (
                    t.def.key,
                    TableMarks::of(t.def.key, t.def.columns, &t.def.marks),
                )
            })
            .collect();
        // At most one table of each role among the tables of the stack, and among the tables of each component
        // (`<component>.<table>`): a stack that joins several decoders has the log of each.
        for role in TableRole::ALL {
            let with: Vec<&str> = marks
                .iter()
                .filter(|(_, m)| m.role == Some(role))
                .map(|(k, _)| *k)
                .collect();
            let owner = |k: &str| k.rsplit_once('.').map_or("", |(c, _)| c).to_owned();
            let mut owners: Vec<String> = with.iter().map(|k| owner(k)).collect();
            owners.sort();
            if owners.windows(2).any(|w| w[0] == w[1]) {
                problems.push(format!("{}: the role {} is on {with:?}", d.id, role.key()));
            }
        }
        // A table of messages or calls has a time, and a table of messages has a text or names who sent it.
        for (key, m) in &marks {
            let log = matches!(m.role, Some(TableRole::Messages | TableRole::Calls));
            if log && m.time.is_none() {
                problems.push(format!(
                    "{}: the table {key} has a role and no time column",
                    d.id
                ));
            }
            if m.role == Some(TableRole::Messages) && m.text.is_none() && m.sender.is_none() {
                problems.push(format!(
                    "{}: the message table {key} has no text and no sender",
                    d.id
                ));
            }
        }
        if !kinds.contains(&kind) {
            continue;
        }
        seen += 1;
        let has_role = marks.iter().any(|(_, m)| m.role.is_some());
        let excused = WITHOUT.iter().find(|(id, _)| *id == d.id);
        match (has_role, excused) {
            (false, None) => problems.push(format!(
                "{} ({}): no table states the role of its rows",
                d.id,
                sdrkit_core::Choice::key(kind)
            )),
            (true, Some((_, why))) => problems.push(format!(
                "{}: the test excuses it ({why}), and it has a table with a role",
                d.id
            )),
            _ => {}
        }
    }
    assert!(seen >= 50, "{seen} stacks of the five kinds");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
