//! Every stack whose content is messages or calls states which of its tables holds them, so that a front end finds the
//! message log, the call log, the nodes and the groups without the keys of the tables. Every send command states the
//! table that gives its target, so that a front end offers the rows as targets.

use sdrkit_core::interface::StackShape;
use sdrkit_core::{ContentKind, FieldDecl, TableMarks, TableRole, ValueKind};

/// The stacks of the five kinds without a table of messages, calls, nodes or groups, each with the reason.
const WITHOUT: &[(&str, &str)] = &[];

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
        for role in TableRole::ALL.into_iter().filter(|r| r.single()) {
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
        // The columns of a delivery have the kinds that a front end reads them by.
        for t in &shape.tables {
            let m = TableMarks::of(t.def.key, t.def.columns, &t.def.marks);
            for (name, column, kind) in [
                ("own", m.own, ValueKind::Bool),
                ("delivery", m.delivery, ValueKind::Choice),
                ("failure", m.failure, ValueKind::Text),
            ] {
                let Some(c) = column else { continue };
                let c = t.def.columns[usize::from(c)];
                if c.kind() != kind {
                    problems.push(format!(
                        "{}: the {name} column {} of the table {} is {:?}, not {kind:?}",
                        d.id,
                        c.key(),
                        t.def.key,
                        c.kind()
                    ));
                }
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

/// The names of an argument that says to whom or where a command sends.
const TARGET_NAMES: &[&str] = &[
    "to",
    "dest",
    "destination",
    "target",
    "address",
    "channel",
    "group",
    "talkgroup",
    "node",
];

/// The target arguments of send commands that name no table, each as (stack, command, argument) with the reason.
/// `*` stands for every stack that lists the command.
const NO_TABLE: &[(&str, &str, &str, &str)] = &[
    (
        "ism",
        "send",
        "channel",
        "the channel is a number inside the frame of a sensor, not an addressee",
    ),
    (
        "bletracker",
        "send",
        "address",
        "the stack lists no advertiser address: trackers change it, and the table names them by identifier",
    ),
];

/// The declarations of `shape` that can name a table: the arguments of its commands as (`<command>.<argument>`,
/// declaration, the command transmits), then its settings as (`setting.<key>`, declaration, false).
fn referring(shape: &StackShape) -> Vec<(String, FieldDecl, bool)> {
    let mut out = Vec::new();
    for c in &shape.commands {
        let def = c.command.def();
        for a in def.args.settings() {
            out.push((
                format!("{}.{}", c.command.key, a.field.key),
                a.field.clone(),
                def.transmits.is_some(),
            ));
        }
    }
    for s in &shape.settings {
        let d = s.decl();
        out.push((format!("setting.{}", d.field.key), d.field, false));
    }
    out
}

#[test]
fn a_send_command_names_the_table_of_its_target() {
    let mut problems = Vec::new();
    let mut stated = 0;
    let mut used = vec![false; NO_TABLE.len()];
    for d in sdrkit_host_catalog::all() {
        let Some(shape) = sdrkit_host_catalog::family_of(d.id).and_then(|f| f.stack_shape(d.id))
        else {
            continue;
        };
        for (path, field, transmits) in referring(shape) {
            let (owner, key) = path.rsplit_once('.').unwrap_or(("", &path));
            let refers = field.refers_to.as_ref();
            // A value column is a column of a table of the stack, of the kind of the argument.
            if let Some(r) = refers.filter(|r| !r.column.is_empty()) {
                stated += 1;
                let table = shape.tables.iter().find(|t| t.def.key == r.table);
                let column =
                    table.and_then(|t| t.def.columns.iter().find(|c| c.key() == r.column.as_ref()));
                match (table, column) {
                    (None, _) => problems.push(format!(
                        "{}: {path} names the table {}, which the stack does not have",
                        d.id, r.table
                    )),
                    (Some(_), None) => problems.push(format!(
                        "{}: {path} names the column {} of the table {}, which the table does not have",
                        d.id, r.column, r.table
                    )),
                    (Some(_), Some(c)) if c.kind() != field.kind => problems.push(format!(
                        "{}: {path} is {:?} and the column {} of the table {} is {:?}",
                        d.id,
                        field.kind,
                        r.column,
                        r.table,
                        c.kind()
                    )),
                    _ => {}
                }
            }
            if !transmits || !TARGET_NAMES.contains(&key) {
                continue;
            }
            let excuse = NO_TABLE
                .iter()
                .position(|(s, c, a, _)| (*s == "*" || *s == d.id) && *c == owner && *a == key);
            match (refers, excuse) {
                (None, None) => problems.push(format!(
                    "{}: the argument {key} of the send command {owner} names no table",
                    d.id
                )),
                (Some(_), Some(i)) => problems.push(format!(
                    "{}: the test excuses {path} ({}), and it names a table",
                    d.id, NO_TABLE[i].3
                )),
                (None, Some(i)) => used[i] = true,
                (Some(_), None) => {}
            }
        }
    }
    for (i, (s, c, a, _)) in NO_TABLE.iter().enumerate() {
        if !used[i] {
            problems.push(format!(
                "the test excuses {s} {c}.{a}, which is no target argument of a send command"
            ));
        }
    }
    assert!(
        stated >= 10,
        "{stated} arguments and settings with a value column"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
