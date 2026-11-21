//! The catalogs of the engine say what a stack transmits: the table `stack_transmits` has each command that transmits,
//! what it sends and the command that stops a transmission that lasts, and the stacks table marks the stack.

mod common;

use common::*;
use sdrkit_core::interface::{Family, StackShape};
use sdrkit_core::{Choice, Clock, Command, Facilities, MemStorage, Random, TxContent, Value};
use sdrkit_host_engine::{Engine, OwnerRef, Setup};
use std::sync::Arc;

static CQ: Command = Command::new("cq", "Call CQ")
    .transmits(TxContent::Text)
    .stopped_by("halt");
static ONCE: Command = Command::new("once", "Send once").transmits(TxContent::Beacon);
static HALT: Command = Command::new("halt", "Halt");

fn cell(e: &Engine, table: &str, row: &str, col: &str) -> Option<Value> {
    e.owner(OwnerRef::Engine, |o| {
        let res = o.results();
        let t = res.table(res.decl().table_id(table)?)?;
        let c = t.columns().id(col)?;
        t.row(row)?.value(c).cloned()
    })
    .unwrap()
}

#[test]
fn the_table_of_transmit_commands_names_the_stop_command() {
    let family: &'static Family = Box::leak(Box::new(
        Family::new("meters", 1, 0).stack(
            StackShape::new("meter")
                .command(&CQ)
                .command(&ONCE)
                .command(&HALT),
        ),
    ));
    let e = Engine::new(Setup {
        facilities: Facilities {
            storage: Arc::new(MemStorage::new()),
            clock: Clock::fixed(1.7e9),
            random: Random::fixed(7),
            codecs: Arc::default(),
        },
        transport: Arc::new(sdrkit_core::net::FakeTransport::new()),
        stacks: vec![&METER, &VOICE],
        families: vec![family],
        components: sdrkit_graph::Registry::standard(),
    })
    .unwrap();
    assert_eq!(
        cell(&e, "stacks", "meter", "transmit"),
        Some(Value::Bool(true))
    );
    assert_eq!(
        cell(&e, "stacks", "voice", "transmit"),
        Some(Value::Bool(false))
    );
    assert_eq!(
        cell(&e, "stack_transmits", "meter/cq", "content"),
        Some(Value::Choice(TxContent::Text.index()))
    );
    assert_eq!(
        cell(&e, "stack_transmits", "meter/cq", "stopped_by"),
        Some(Value::Text("halt".into()))
    );
    assert_eq!(
        cell(&e, "stack_transmits", "meter/once", "content"),
        Some(Value::Choice(TxContent::Beacon.index()))
    );
    assert!(matches!(
        cell(&e, "stack_transmits", "meter/once", "stopped_by"),
        None | Some(Value::None)
    ));
    assert_eq!(cell(&e, "stack_transmits", "meter/halt", "content"), None);
}
