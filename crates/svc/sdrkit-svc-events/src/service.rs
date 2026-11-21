//! Small parts that every event service uses to be an owner: row keys of its tables and the errors of its commands.

use sdrkit_core::command::{CommandError, CommandId, Commands};
use sdrkit_core::publish::{field, Published, TableDef};
use sdrkit_core::settings::Argument;
use sdrkit_core::{Record, SettingError, StageError, StorePath, TableId, Values};

/// The command error for a failure of the owner's own typed error `e`.
pub fn failed(e: impl std::error::Error + Send + Sync + 'static) -> CommandError {
    CommandError::Failed {
        source: StageError::new(e),
    }
}

/// The id of table `t` in `p`. `TableId(u16::MAX)` if `p` does not declare it.
pub fn table_id(p: &Published, t: &TableDef) -> TableId {
    p.results()
        .decl()
        .table_id(t.key)
        .unwrap_or(TableId(u16::MAX))
}

/// [`CommandError::NoSuchRow`] for row `key` of table `t`.
pub fn no_such_row(p: &Published, t: &TableDef, key: &str) -> CommandError {
    CommandError::NoSuchRow {
        table: table_id(p, t),
        key: key.to_owned(),
    }
}

/// True if table `t` has row `key`.
pub fn has_row(p: &Published, t: &TableDef, key: &str) -> bool {
    p.results()
        .table(table_id(p, t))
        .is_some_and(|table| table.row(key).is_some())
}

/// An empty reply record of command `id`.
///
/// # Errors
///
/// [`CommandError::NoSuchId`] if `commands` does not declare `id`.
pub fn reply(commands: &Commands, id: CommandId) -> Result<Record, CommandError> {
    commands
        .def(id)
        .map(|d| d.new_reply())
        .ok_or(CommandError::NoSuchId { id })
}

/// The file that the required text argument `a` names in the host's storage.
///
/// # Errors
///
/// [`CommandError::BadArgument`] with [`SettingError::Malformed`] if the text is not a [`StorePath`]. What
/// [`Values::arg`] returns if the argument is missing.
pub fn path_arg(args: &Values, a: &Argument<field::Text>) -> Result<StorePath, CommandError> {
    let text = args.arg(a)?;
    StorePath::new(text.trim()).map_err(|_| {
        let arg = args.schema().id(a.field.key).ok();
        match arg {
            Some(arg) => CommandError::BadArgument {
                arg,
                error: SettingError::Malformed { id: arg, at: None },
            },
            None => CommandError::NotAnArgument { key: a.field.key },
        }
    })
}
