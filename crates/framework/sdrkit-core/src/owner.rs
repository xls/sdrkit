//! Owners: anything a host can configure, read or command (the engine, a radio, a session, a stack, a component, a service).
//! Every owner declares its settings, results, events and commands through one trait ([`Owner`]).

use crate::command::{CommandError, CommandId, Commands};
use crate::event::EventDecl;
use crate::record::Record;
use crate::results::{Results, ResultsDecl};
use crate::settings::{Schema, SettingError, SettingId, Values, ValuesError};
use crate::stack::ConfigError;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::sync::{Arc, LazyLock};

/// The lookup key of an owner (`"service.sweep"`, `"stack.dab"`): one or more parts joined by `.`. Each part starts with a
/// lowercase ASCII letter, followed by lowercase ASCII letters, digits, `_` and `-` (`"stack.tv-auto"`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OwnerKey(Cow<'static, str>);

impl OwnerKey {
    /// The key `key`.
    ///
    /// # Errors
    ///
    /// [`OwnerKeyError`] if `key` does not follow the rules of [`OwnerKey`].
    pub fn new(key: impl Into<Cow<'static, str>>) -> Result<Self, OwnerKeyError> {
        let key = key.into();
        match Self::first_error(key.as_bytes()) {
            Some(e) => Err(e),
            None => Ok(OwnerKey(key)),
        }
    }

    /// The constant key `key`.
    ///
    /// # Panics
    ///
    /// If `key` does not follow the rules of [`OwnerKey`]. In a `const`, this stops the build.
    pub const fn of(key: &'static str) -> Self {
        assert!(Self::is_valid(key), "not a valid owner key");
        OwnerKey(Cow::Borrowed(key))
    }

    /// True if `key` follows the rules of [`OwnerKey`].
    pub const fn is_valid(key: &str) -> bool {
        Self::first_error(key.as_bytes()).is_none()
    }

    const fn first_error(b: &[u8]) -> Option<OwnerKeyError> {
        if b.is_empty() {
            return Some(OwnerKeyError::Empty);
        }
        let mut i = 0;
        let mut part_start = true;
        while i < b.len() {
            let c = b[i];
            if c == b'.' {
                if part_start {
                    return Some(OwnerKeyError::EmptyPart { at: i });
                }
                part_start = true;
            } else if part_start {
                if !c.is_ascii_lowercase() {
                    return Some(OwnerKeyError::BadByte { at: i });
                }
                part_start = false;
            } else if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_' || c == b'-') {
                return Some(OwnerKeyError::BadByte { at: i });
            }
            i += 1;
        }
        if part_start {
            return Some(OwnerKeyError::EmptyPart { at: b.len() });
        }
        None
    }

    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for OwnerKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for OwnerKey {
    type Error = OwnerKeyError;
    fn try_from(s: String) -> Result<Self, OwnerKeyError> {
        Self::new(s)
    }
}

impl From<OwnerKey> for String {
    fn from(k: OwnerKey) -> Self {
        k.0.into_owned()
    }
}

/// Why a text is not an [`OwnerKey`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum OwnerKeyError {
    /// The key is empty.
    #[error("the owner key is empty")]
    Empty,
    /// A part between two dots, or at one end, is empty.
    #[error("the owner key has an empty part at byte {at}")]
    EmptyPart {
        /// The byte offset of the empty part.
        at: usize,
    },
    /// A byte is not allowed at its place.
    #[error("the owner key has a byte that is not allowed at byte {at}")]
    BadByte {
        /// The byte offset.
        at: usize,
    },
}

static NO_SETTINGS: LazyLock<Arc<Schema>> = LazyLock::new(Arc::default);
static NO_RESULTS: LazyLock<Arc<ResultsDecl>> = LazyLock::new(Arc::default);
static EMPTY_RESULTS: LazyLock<Results> = LazyLock::new(Results::default);
static NO_COMMANDS: LazyLock<Commands> = LazyLock::new(Commands::default);

/// Anything a host can configure, read or command. An owner declares four things: settings, results, events and commands.
/// Each has a default body for an owner that has none of it.
///
/// A host calls [`apply`] and [`invoke`] of this module, which make sure that the values fit
/// before the owner's methods.
pub trait Owner: Send {
    /// The settings. The default has none.
    fn schema(&self) -> &Arc<Schema> {
        &NO_SETTINGS
    }
    /// The values of the settings. `None` (the default) for an owner with no values, such as a kind in a catalog.
    fn values(&self) -> Option<&Values> {
        None
    }
    /// Applies a new value of setting `id`. [`apply`] has passed the value through [`Schema::check`].
    ///
    /// # Errors
    ///
    /// [`SettingError`] if the owner refuses the value. Its state does not change. The default returns
    /// [`SettingError::NoSuchId`].
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        let _ = value;
        Err(SettingError::NoSuchId { id })
    }
    /// The declaration of the results. The default declares nothing.
    fn results_decl(&self) -> &Arc<ResultsDecl> {
        &NO_RESULTS
    }
    /// The results. The default returns empty results with an empty declaration.
    fn results(&self) -> &Results {
        &EMPTY_RESULTS
    }
    /// The event kinds. The default returns those of [`Owner::results_decl`].
    fn events(&self) -> &[EventDecl] {
        self.results_decl().events()
    }
    /// The commands. The default has none.
    fn commands(&self) -> &Commands {
        &NO_COMMANDS
    }
    /// Runs command `id` with arguments that fit its declaration ([`invoke`] makes sure of it). The command returns at once. Long work runs as a job.
    /// The reply is `None` for a command with an empty reply.
    ///
    /// # Errors
    ///
    /// [`CommandError`] if the owner refuses the command or it fails. The default returns [`CommandError::NoSuchId`].
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        let _ = args;
        Err(CommandError::NoSuchId { id })
    }
    /// Shows `text` (a request of this owner that the engine refused) in the status and the log. The default does nothing.
    fn problem(&mut self, text: &str) {
        let _ = text;
    }
}

/// Applies `value` to setting `id` of `owner` after [`Schema::check`].
///
/// # Errors
///
/// What [`Schema::check`] returns, else what [`Owner::apply`] returns.
pub fn apply<O: Owner + ?Sized>(
    owner: &mut O,
    id: SettingId,
    value: &Value,
) -> Result<(), SettingError> {
    owner.schema().check(id, value)?;
    owner.apply(id, value)
}

/// Runs command `id` of `owner` with the arguments `given`, after [`Commands::check`]. A slot of `given` at [`Value::None`]
/// takes its default.
///
/// # Errors
///
/// What [`Commands::check`] returns, else what [`Owner::invoke`] returns.
pub fn invoke<O: Owner + ?Sized>(
    owner: &mut O,
    id: CommandId,
    given: &Record,
) -> Result<Option<Record>, CommandError> {
    let args = owner.commands().check(id, given)?;
    owner.invoke(id, &args)
}

/// The label and key of setting or argument `id` of `schema` as a message shows them: `"Sample rate" (rate)`.
fn named(schema: &Schema, id: SettingId) -> String {
    match schema.setting(id) {
        Some(d) => format!("\"{}\" ({})", d.field.label, d.field.key),
        None => format!("number {}", id.0),
    }
}

/// The first sentence of `help`, without its full stop.
fn first_sentence(help: &str) -> &str {
    let end = help.find(". ").unwrap_or(help.len());
    help[..end].trim_end_matches('.')
}

/// `e` as one English sentence that names each setting by its label and key in `schema`. `noun` is what a slot of
/// `schema` is called: `setting`, or `argument` for the arguments of a command. A host shows this text, never the
/// `Display` of the error, which has only the number of the setting.
pub fn setting_error_text(schema: &Schema, noun: &str, e: &SettingError) -> String {
    let n = |id: &SettingId| named(schema, *id);
    match e {
        SettingError::Unknown { .. } => e.to_string(),
        SettingError::NoSuchId { id } => format!("there is no {noun} with number {}", id.0),
        SettingError::WrongKind { id, expected, got } => {
            format!("{noun} {} takes {expected:?}, not {got:?}", n(id))
        }
        SettingError::Required { id } => format!("{noun} {} needs a value", n(id)),
        SettingError::OutOfRange { id, min, max, got } => {
            let unit = schema.setting(*id).map_or("", |d| d.field.unit.as_ref());
            let sp = if unit.is_empty() { "" } else { " " };
            format!(
                "{got}{sp}{unit} is outside {min} to {max}{sp}{unit} for {noun} {}",
                n(id)
            )
        }
        SettingError::NotFinite { id } => format!("{noun} {} needs a finite number", n(id)),
        SettingError::NotAChoice { id, index } => {
            format!("{noun} {} has no choice {index}", n(id))
        }
        SettingError::NotPermitted { id } => {
            format!("{noun} {} does not allow this value", n(id))
        }
        SettingError::Malformed { id, at } => {
            let at = at.map_or_else(String::new, |at| format!(" at byte {at}"));
            let form = schema.setting(*id).map_or("", |d| first_sentence(&d.help));
            let form = if form.is_empty() {
                String::new()
            } else {
                format!(". It takes: {form}")
            };
            format!("the text of {noun} {} is not valid{at}{form}", n(id))
        }
        SettingError::Fixed { id } => format!("{noun} {} is fixed", n(id)),
        SettingError::Conflict { id, with } => {
            format!("{noun} {} does not agree with {noun} {}", n(id), n(with))
        }
        SettingError::Unsupported { id } => format!("{noun} {} is not used now", n(id)),
        SettingError::NeedsStation { id, item } => {
            format!("{noun} {} needs the station's {item:?}", n(id))
        }
        SettingError::Storage { id, path, source } => {
            format!("{noun} {}: {path}: {source}", n(id))
        }
        SettingError::Config { id, source } => format!("{noun} {}: {source}", n(id)),
    }
}

/// `e` as one English sentence that names its setting by its label and key in `schema`, the schema of the setting ids
/// that it holds. A host shows this text, never the `Display` of the error, which has only the number of the setting.
pub fn values_error_text(schema: &Schema, e: &ValuesError) -> String {
    match e {
        ValuesError::NotAnObject => e.to_string(),
        ValuesError::WrongJson { id, expected, got } => format!(
            "setting {} takes {expected:?}, not a JSON {got:?}",
            named(schema, *id)
        ),
        ValuesError::UnknownChoice { id, key } => {
            format!("setting {} has no choice {key}", named(schema, *id))
        }
        ValuesError::Setting(e) => setting_error_text(schema, "setting", e),
    }
}

/// `e` as one English sentence that names its setting by its label and key in `schema`, the schema of the setting ids
/// that it holds, for the errors that refer to a setting. Any other error is its `Display`.
pub fn config_error_text(schema: &Schema, e: &ConfigError) -> String {
    match e {
        ConfigError::Setting(e) => setting_error_text(schema, "setting", e),
        ConfigError::SettingAtRate { id, rate_hz } => format!(
            "setting {} cannot work at {rate_hz} samples a second",
            named(schema, *id)
        ),
        e => e.to_string(),
    }
}

/// `e` of command `id` of `owner` as one English sentence that names the command, its arguments, the result field and
/// the table by their labels and keys. A host shows this text, never the `Display` of the error.
pub fn command_error_text<O: Owner + ?Sized>(owner: &O, id: CommandId, e: &CommandError) -> String {
    let def = owner.commands().def(id);
    let command = def.map_or_else(
        || format!("number {}", id.0),
        |d| format!("\"{}\" ({})", d.label, d.key),
    );
    let arg = |a: &SettingId| match def {
        Some(d) => named(&d.args, *a),
        None => format!("number {}", a.0),
    };
    match e {
        CommandError::Unknown { .. }
        | CommandError::NoSuchTable { .. }
        | CommandError::NotAnArgument { .. } => e.to_string(),
        CommandError::NoSuchId { id } => format!("there is no command with number {}", id.0),
        CommandError::ArgumentsDecl { .. } => {
            format!("the arguments are not those of command {command}")
        }
        CommandError::MissingArgument { arg: a } => {
            format!("command {command} needs its argument {}", arg(a))
        }
        CommandError::BadArgument { error, .. } => match def {
            Some(d) => format!(
                "command {command}: {}",
                setting_error_text(&d.args, "argument", error)
            ),
            None => format!("command {command}: an argument is refused"),
        },
        CommandError::NotNow { field, state } => {
            let results = owner.results().decl();
            let name = results.fields().field(*field).map_or_else(
                || format!("number {}", field.0),
                |f| format!("\"{}\"", f.label),
            );
            format!("command {command} is refused now: {name} is {state:?}")
        }
        CommandError::NoSuchRow { table, key } => {
            let results = owner.results().decl();
            let name = results.table(*table).map_or_else(
                || format!("number {}", table.0),
                |t| format!("\"{}\"", t.label),
            );
            format!("command {command}: table {name} has no row {key}")
        }
        CommandError::NoSuchKey { arg: a } => {
            format!(
                "command {command}: argument {} refers to something that does not exist",
                arg(a)
            )
        }
        CommandError::TransmitOff => format!("command {command} is refused: {e}"),
        CommandError::Failed { source } => format!("command {command} failed: {source}"),
    }
}
