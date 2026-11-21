//! The control port: a later component (a supervisor, a display, a frame decoder) sends settings to an earlier one over a
//! feedback edge.
//!
//! The controller has an output `Port::command("control")` and the controlled component an input of the same name. The controlled
//! component applies the [`Command`] items it receives with [`apply_commands`] or, if a command can change a rate, with
//! [`apply_commands_reformat`]. A command sent in one pass takes effect at the start of the next.

use crate::component::{Applied, Component, Io};
use crate::stream::{Item, Timed};
use sdrkit_core::{SettingError, SettingId, Value};

/// One instruction to a component: the id of a setting in the receiving component's schema, and the value. An action is a setting
/// of the component too (a flag that it acts on).
#[derive(Clone, Debug, PartialEq)]
pub struct Command {
    /// The setting id, as the receiving component's [`Component::apply`] expects it.
    pub id: SettingId,
    /// The value.
    pub value: Value,
}

impl Command {
    /// A command.
    pub fn new(id: SettingId, value: Value) -> Command {
        Command { id, value }
    }
}

/// Checks `c` against the schema of `comp` and applies it.
fn apply_one<C: Component + ?Sized>(
    comp: &mut C,
    schema: &sdrkit_core::Schema,
    c: &Command,
) -> Result<Applied, SettingError> {
    schema.check(c.id, &c.value)?;
    comp.apply(c.id, &c.value)
}

impl Item for Command {
    const KIND: &'static str = "command";
}

/// Applies the commands waiting on input `port` through `comp.apply`, in order, each validated against the component's schema first.
/// Returns the first refusal (later commands are still applied), so a component can log it.
pub fn apply_commands<C: Component + ?Sized>(
    comp: &mut C,
    io: &Io,
    port: usize,
) -> Result<(), SettingError> {
    let mut first = Ok(());
    let cmds: &[Timed<Command>] = io.items::<Command>(port);
    if cmds.is_empty() {
        return first;
    }
    let schema = io.schema();
    for c in cmds {
        if let Err(e) = apply_one(comp, schema, &c.value) {
            if first.is_ok() {
                first = Err(e);
            }
        }
    }
    first
}

/// Applies the commands waiting on input `port` through `comp.apply`, in order, as [`apply_commands`] does. When a command returns
/// [`Applied::Reformat`], it requests that the graph configure the component again at the end of the pass
/// ([`Io::request_reformat`]). The outputs of this pass keep the old formats, so the component keeps the new value aside and
/// takes it in its next [`Component::configure`]. Returns the first refusal (later commands are still applied).
pub fn apply_commands_reformat<C: Component + ?Sized>(
    comp: &mut C,
    io: &mut Io,
    port: usize,
) -> Result<(), SettingError> {
    let mut first = Ok(());
    let cmds: &[Timed<Command>] = io.items::<Command>(port);
    if cmds.is_empty() {
        return first;
    }
    let schema = io.schema();
    let mut reformat = false;
    for c in cmds {
        match apply_one(comp, schema, &c.value) {
            Ok(Applied::Reformat) => reformat = true,
            Ok(Applied::Live) => {}
            Err(e) => {
                if first.is_ok() {
                    first = Err(e);
                }
            }
        }
    }
    if reformat {
        io.request_reformat();
    }
    first
}

/// The component commands that a command alias of a graph names ([`GraphBuilder::command_alias`](crate::GraphBuilder::command_alias)).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandTarget {
    /// One command of one component.
    One {
        /// The component id.
        component: String,
        /// The command key in the component's commands.
        command: String,
    },
    /// The command of this key of every component that declares it (`clear`: every decoder).
    Every(String),
}

impl CommandTarget {
    /// Command `command` of component `component`.
    pub fn of(component: &str, command: &str) -> CommandTarget {
        CommandTarget::One {
            component: component.into(),
            command: command.into(),
        }
    }
    /// Command `command` of every component that declares it.
    pub fn every(command: &str) -> CommandTarget {
        CommandTarget::Every(command.into())
    }
    /// The command key.
    pub fn command(&self) -> &str {
        match self {
            CommandTarget::One { command, .. } | CommandTarget::Every(command) => command,
        }
    }
    /// The component, for a target of one component.
    pub fn component(&self) -> Option<&str> {
        match self {
            CommandTarget::One { component, .. } => Some(component),
            CommandTarget::Every(_) => None,
        }
    }
}

impl std::fmt::Display for CommandTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandTarget::One { component, command } => {
                write!(f, "command \"{command}\" of component \"{component}\"")
            }
            CommandTarget::Every(command) => write!(f, "every command \"{command}\""),
        }
    }
}
