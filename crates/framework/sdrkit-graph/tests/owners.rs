//! Graphs, stack graphs and their components as owners: commands, aliases, change counters and access by handle.

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::owner::{self, Owner};
use sdrkit_core::publish::{field, Field, Published};
use sdrkit_core::{
    Argument, ConfigError, InputInfo, Output, Receiver, Record, Results, SettingId, Values,
};
use sdrkit_graph::{
    no_values, Component, ComponentHandle, ComponentInfo, Format, GraphBuilder, GraphError,
    GraphPlan, Io, Port, StackGraph,
};

const TOTAL: Field<field::I64> = Field::new("total", "Total", "");
const AMOUNT: Field<field::I64> = Field::new("amount", "Amount", "");
const ADD: Command = Command::new("add", "Add").args(&[&Argument::new(AMOUNT)]);
const RESET: Command = Command::new("total.reset", "Reset the total");

sdrkit_core::commands! {
    enum CounterCommand {
        Add = ADD,
        Reset = RESET,
    }
}

/// A sink that adds the amount of each `add` command to its total.
struct Counter {
    total: i64,
    results: Published,
}

impl Counter {
    fn new() -> Self {
        Counter {
            total: 0,
            results: Published::new(&[&TOTAL]),
        }
    }
}

impl Component for Counter {
    fn info(&self) -> ComponentInfo {
        ComponentInfo::sink("test.counter", vec![Port::iq("in")])
    }
    fn configure(&mut self, _: &[Format]) -> Result<Vec<Format>, ConfigError> {
        Ok(vec![])
    }
    fn process(&mut self, _: &mut Io) {
        self.results.set(&TOTAL, self.total);
    }
    fn values(&self) -> Values {
        no_values()
    }
    fn results(&self) -> Option<&Results> {
        Some(self.results.results())
    }
    fn commands(&self) -> &sdrkit_core::Commands {
        CounterCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        match CounterCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            CounterCommand::Add => self.total += args.arg(&Argument::new(AMOUNT))?,
            CounterCommand::Reset => self.total = 0,
        }
        Ok(None)
    }
}

fn counter_graph() -> sdrkit_graph::Graph {
    GraphBuilder::new()
        .add("count", Counter::new())
        .input("count")
        .build()
        .unwrap()
}

fn add_args(g: &impl Owner, id: CommandId, amount: Option<i64>) -> Record {
    let mut args = g.commands().def(id).unwrap().new_args();
    if let Some(a) = amount {
        args.set(&AMOUNT, a);
    }
    args
}

#[test]
fn a_component_command_runs_through_the_graph_with_checked_arguments() {
    let mut g = counter_graph();
    let id = g.commands().id("count.add").unwrap();
    let none = add_args(&g, id, None);
    assert_eq!(
        owner::invoke(&mut g, id, &none),
        Err(CommandError::MissingArgument { arg: SettingId(0) })
    );
    let five = add_args(&g, id, Some(5));
    owner::invoke(&mut g, id, &five).unwrap();
    g.configure(&InputInfo::new(1000.0, 0.0, 0.0)).unwrap();
    g.feed(&[Default::default(); 10], &mut Output::default());
    g.refresh_results();
    let total = g.results().decl().field_id("count.total").unwrap();
    assert_eq!(
        g.results().fields().value(total),
        Some(&sdrkit_core::Value::I64(5))
    );
}

#[test]
fn a_stack_command_alias_resolves_at_build() {
    let mut rx = StackGraph::builder("count", counter_graph())
        .command("plus", sdrkit_graph::CommandTarget::of("count", "add"))
        .build()
        .unwrap();
    let id = rx.commands().id("plus").unwrap();
    assert_eq!(rx.commands().len(), 1);
    let two = add_args(&rx, id, Some(2));
    owner::invoke(&mut rx, id, &two).unwrap();

    let e = StackGraph::builder("count", counter_graph())
        .command("plus", sdrkit_graph::CommandTarget::of("count", "nothing"))
        .build()
        .unwrap_err();
    assert!(matches!(e, GraphError::UnresolvedCommand { .. }));
}

#[test]
fn a_graph_that_publishes_equal_values_moves_no_counter() {
    let mut g = counter_graph();
    g.configure(&InputInfo::new(1000.0, 0.0, 0.0)).unwrap();
    let mut out = Output::default();
    g.feed(&[Default::default(); 10], &mut out);
    g.refresh_results();
    let seen = g.results().change_counter();
    g.feed(&[Default::default(); 10], &mut out);
    g.refresh_results();
    g.refresh_results();
    assert_eq!(g.results().change_counter(), seen);
}

#[test]
fn a_component_is_an_owner_through_its_handle() {
    let r = sdrkit_graph::Registry::standard();
    let mut p = GraphPlan::new();
    let chan = p.add(&r, "chan", "dsp.channel").unwrap();
    p.input(sdrkit_graph::PortRef::new(chan, 0)).unwrap();
    let mut g = p.build().unwrap();
    let mut c = g.owner(chan).unwrap();
    let rate = c.schema().id("rate").unwrap();
    owner::apply(&mut c, rate, &sdrkit_core::Value::Hz(24e3)).unwrap();
    assert_eq!(
        c.values().unwrap().value(rate),
        Some(&sdrkit_core::Value::Hz(24e3))
    );
    assert!(c.commands().is_empty());
    assert!(g.owner(ComponentHandle(7)).is_none());
}

#[test]
fn a_description_keeps_the_command_aliases() {
    let g = GraphBuilder::new()
        .add("count", Counter::new())
        .input("count")
        .command_alias("plus", sdrkit_graph::CommandTarget::of("count", "add"))
        .build()
        .unwrap();
    let d = g.describe();
    assert_eq!(
        d.command_aliases.get("plus"),
        Some(&sdrkit_graph::CommandTarget::of("count", "add"))
    );
    let mut r = sdrkit_graph::Registry::new();
    r.add("test.counter", || Box::new(Counter::new()));
    let back = sdrkit_graph::Description::from_json(&d.to_json()).unwrap();
    let g2 = back.build(&r).unwrap();
    assert!(g2.commands().id("plus").is_ok());
    let mut bad = back.clone();
    bad.command_aliases.insert(
        "minus".into(),
        sdrkit_graph::CommandTarget::of("count", "nothing"),
    );
    assert!(bad.build(&r).is_err());
}

#[test]
fn a_command_alias_reaches_a_command_key_with_a_dot() {
    let g = GraphBuilder::new()
        .add("count", Counter::new())
        .input("count")
        .command_alias(
            "reset",
            sdrkit_graph::CommandTarget::of("count", "total.reset"),
        )
        .command_alias(
            "reset_all",
            sdrkit_graph::CommandTarget::every("total.reset"),
        )
        .build()
        .unwrap();
    assert!(g.commands().id("reset").is_ok());
    assert!(g.commands().id("reset_all").is_ok());
    let json = g.describe().to_json();
    assert!(json.contains("total.reset"), "{json}");
}
