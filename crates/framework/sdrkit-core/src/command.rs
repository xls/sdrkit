//! Commands: the actions an owner declares, with typed arguments and a reply. An owner declares them once as an enum
//! ([`commands!`](crate::commands!)), and the framework makes sure that the arguments fit before the owner runs the command.

use crate::publish::Declare;
use crate::record::{DeclError, FieldId, Record, RecordDecl};
use crate::results::TableId;
use crate::settings::{DeclareSetting, Schema, SettingError, SettingId, Values};
use crate::stack::StageError;
use crate::value::Value;
use serde::Serialize;
use std::borrow::Cow;
use std::sync::Arc;

/// The index of a command in its owner's [`Commands`].
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, serde::Deserialize,
)]
pub struct CommandId(pub u16);

impl CommandId {
    /// The index in the list.
    pub fn index(self) -> usize {
        usize::from(self.0)
    }
}

crate::choice_enum! {
    /// What a command that transmits sends ([`Command::transmits`]).
    pub enum TxContent {
        /// A text message.
        Text = ("text", "Text"),
        /// Speech.
        Voice = ("voice", "Voice"),
        /// A beacon or an announcement of the station.
        Beacon = ("beacon", "Beacon"),
        /// A file, a picture or a stream of media.
        File = ("file", "File"),
        /// A frame or a request of the protocol.
        Frame = ("frame", "Frame"),
        /// An unmodulated carrier, to tune an antenna or an amplifier.
        Carrier = ("carrier", "Carrier"),
    }
}

/// One command, named once as a `const`: its key, what people see, its arguments and its reply.
#[derive(Clone, Copy)]
pub struct Command {
    /// The key (`"sweep.start"`).
    pub key: &'static str,
    /// What people see.
    pub label: &'static str,
    /// One or two plain sentences for people.
    pub help: &'static str,
    /// The arguments, declared as settings are, or as a required [`Argument`](crate::settings::Argument).
    pub args: &'static [&'static dyn DeclareSetting],
    /// The fields of the reply. Empty for most commands.
    pub reply: &'static [&'static dyn Declare],
    /// What the command sends on the air, or `None` for a command that does not transmit. The engine refuses a command
    /// that transmits while the transmitter of its session is off.
    pub transmits: Option<TxContent>,
    /// For a command that transmits: the key of the command of the same owner that stops the transmission that this one
    /// starts, or `None` for a transmission that ends by itself.
    pub stopped_by: Option<&'static str>,
}

impl Command {
    /// A command `key` shown as `label`, with no arguments and an empty reply.
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        Command {
            key,
            label,
            help: "",
            args: &[],
            reply: &[],
            transmits: None,
            stopped_by: None,
        }
    }
    /// The same command with this help text.
    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }
    /// The same command with these arguments.
    #[must_use]
    pub const fn args(mut self, args: &'static [&'static dyn DeclareSetting]) -> Self {
        self.args = args;
        self
    }
    /// The same command with these reply fields.
    #[must_use]
    pub const fn reply(mut self, reply: &'static [&'static dyn Declare]) -> Self {
        self.reply = reply;
        self
    }
    /// The same command, declared as one that transmits `content`.
    #[must_use]
    pub const fn transmits(mut self, content: TxContent) -> Self {
        self.transmits = Some(content);
        self
    }
    /// The same command, whose transmission lasts until the command `key` of the same owner stops it.
    #[must_use]
    pub const fn stopped_by(mut self, key: &'static str) -> Self {
        self.stopped_by = Some(key);
        self
    }
    /// Its declaration.
    ///
    /// # Panics
    ///
    /// If [`Schema::arguments`] refuses the arguments or two reply fields share a key. A constant list that is refused is a
    /// programming error.
    pub fn def(&self) -> CommandDef {
        CommandDef {
            key: Cow::Borrowed(self.key),
            label: Cow::Borrowed(self.label),
            help: Cow::Borrowed(self.help),
            args: Arc::new(Schema::of_arguments(self.args)),
            reply: Arc::new(RecordDecl::of(self.reply)),
            transmits: self.transmits,
            stopped_by: self.stopped_by.map(Cow::Borrowed),
        }
    }
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command").field("key", &self.key).finish()
    }
}

/// The declaration of a command, as data.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CommandDef {
    /// The key.
    pub key: Cow<'static, str>,
    /// What people see.
    pub label: Cow<'static, str>,
    /// One or two plain sentences for people.
    pub help: Cow<'static, str>,
    /// The arguments, with their kinds, ranges, choices and defaults, made by [`Schema::arguments`].
    pub args: Arc<Schema>,
    /// The fields of the reply. Empty for most commands.
    pub reply: Arc<RecordDecl>,
    /// What the command sends on the air, or `None` for a command that does not transmit.
    #[serde(serialize_with = "content_key")]
    pub transmits: Option<TxContent>,
    /// For a command that transmits: the key of the command that stops its transmission, or `None` for a transmission
    /// that ends by itself.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_by: Option<Cow<'static, str>>,
}

/// Writes the key of the content, or nothing.
fn content_key<S: serde::Serializer>(c: &Option<TxContent>, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_some(&c.map(crate::Choice::key))
}

impl CommandDef {
    /// An empty argument record for this command, to fill by [`FieldId`] (the [`SettingId`] of the argument). A slot left at
    /// [`Value::None`] takes its default.
    pub fn new_args(&self) -> Record {
        Record::new(Arc::clone(self.args.record_decl()))
    }

    /// The arguments of command `id` from `given`. Every present value must pass [`Schema::check`]. Every absent value
    /// ([`Value::None`]) takes its default.
    ///
    /// # Errors
    ///
    /// [`CommandError::ArgumentsDecl`] if `given` is not built on the argument declaration. [`CommandError::BadArgument`] for
    /// the first argument that is refused. [`CommandError::MissingArgument`] for the first required argument left out.
    pub fn check(&self, id: CommandId, given: &Record) -> Result<Values, CommandError> {
        let decl = self.args.record_decl();
        if !Arc::ptr_eq(given.decl(), decl) && **given.decl() != **decl {
            return Err(CommandError::ArgumentsDecl { id });
        }
        let mut values = Values::defaults(&self.args);
        for (i, (_, v)) in given.iter().enumerate() {
            if matches!(v, Value::None) {
                continue;
            }
            let arg = SettingId(u16::try_from(i).unwrap_or(u16::MAX));
            values
                .set(arg, v)
                .map_err(|error| CommandError::BadArgument { arg, error })?;
        }
        for (i, s) in self.args.settings().iter().enumerate() {
            let arg = SettingId(u16::try_from(i).unwrap_or(u16::MAX));
            if s.required && values.value(arg).is_none_or(Value::is_none) {
                return Err(CommandError::MissingArgument { arg });
            }
        }
        Ok(values)
    }

    /// An empty reply record for this command, for the owner to fill.
    pub fn new_reply(&self) -> Record {
        Record::new(Arc::clone(&self.reply))
    }
}

/// The commands of one owner, in the order a front end shows them. [`CommandId`] is the index in this list.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Commands {
    defs: Vec<CommandDef>,
}

impl Commands {
    /// A list of these commands, for a list built at run time.
    ///
    /// # Errors
    ///
    /// [`DeclError`] if two commands share a key or there are more than 65535.
    pub fn new(defs: Vec<CommandDef>) -> Result<Self, DeclError> {
        if defs.len() > usize::from(u16::MAX) {
            return Err(DeclError::TooMany { count: defs.len() });
        }
        let mut keys: Vec<&str> = defs.iter().map(|d| d.key.as_ref()).collect();
        keys.sort_unstable();
        if let Some(w) = keys.windows(2).find(|w| w[0] == w[1]) {
            return Err(DeclError::DuplicateKey {
                key: w[0].to_owned(),
            });
        }
        Ok(Commands { defs })
    }

    /// A list of constant commands.
    ///
    /// # Panics
    ///
    /// If [`Commands::new`] or [`Command::def`] refuses the list. A constant list that is refused is a programming error.
    pub fn of(commands: &[&Command]) -> Self {
        match Self::new(commands.iter().map(|c| c.def()).collect()) {
            Ok(c) => c,
            Err(e) => panic!("{e}"),
        }
    }

    /// The id of the command with key `key`. This is the only lookup by text. Do it once and keep the id.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unknown`] if the list has no such key.
    pub fn id(&self, key: &str) -> Result<CommandId, CommandError> {
        self.defs
            .iter()
            .position(|d| d.key == key)
            .and_then(|i| u16::try_from(i).ok())
            .map(CommandId)
            .ok_or_else(|| CommandError::Unknown { key: key.into() })
    }

    /// The declaration of command `id`.
    pub fn def(&self, id: CommandId) -> Option<&CommandDef> {
        self.defs.get(id.index())
    }

    /// The declarations, in order.
    pub fn defs(&self) -> &[CommandDef] {
        &self.defs
    }

    /// The number of commands.
    pub fn len(&self) -> usize {
        self.defs.len()
    }

    /// True if there are no commands.
    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// The arguments of command `id` from `given`, as [`CommandDef::check`] returns them.
    ///
    /// # Errors
    ///
    /// [`CommandError::NoSuchId`] if there is no command `id`, else what [`CommandDef::check`] returns.
    pub fn check(&self, id: CommandId, given: &Record) -> Result<Values, CommandError> {
        self.def(id)
            .ok_or(CommandError::NoSuchId { id })?
            .check(id, given)
    }
}

/// A list of commands serialises as the list of its declarations.
impl Serialize for Commands {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.defs.serialize(s)
    }
}

/// The commands of one owner as an enum with one variant per command. Implement it with [`commands!`](crate::commands!).
pub trait CommandSet: Copy + Sized + 'static {
    /// The commands, in variant order.
    const COMMANDS: &'static [&'static Command];
    /// The declarations. They are built once and shared.
    fn commands() -> &'static Commands;
    /// The id of this command.
    fn id(self) -> CommandId;
    /// The command with this id. `None` if the id is outside the set.
    fn from_id(id: CommandId) -> Option<Self>;
    /// The command with key `key`.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unknown`] if no command has this key.
    fn from_key(key: &str) -> Result<Self, CommandError> {
        let id = Self::commands().id(key)?;
        Self::from_id(id).ok_or(CommandError::NoSuchId { id })
    }
}

/// Declares an owner's commands as an enum and implements [`CommandSet`] for it. Each variant names its [`Command`] constant.
///
/// ```
/// use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
/// use sdrkit_core::publish::field;
/// use sdrkit_core::{Record, Setting, Values};
///
/// const FROM: Setting<field::Hz> = Setting::new("from", "From", "Hz", 88e6);
/// const START: Command = Command::new("sweep.start", "Start").args(&[&FROM]);
/// const STOP: Command = Command::new("sweep.stop", "Stop");
///
/// sdrkit_core::commands! {
///     /// The commands of a sweep.
///     pub enum SweepCommand {
///         Start = START,
///         Stop = STOP,
///     }
/// }
///
/// struct Sweep {
///     from_hz: Option<f64>,
/// }
///
/// impl Sweep {
///     fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
///         match SweepCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
///             SweepCommand::Start => self.from_hz = Some(args.get(&FROM)),
///             SweepCommand::Stop => self.from_hz = None,
///         }
///         Ok(None)
///     }
/// }
///
/// let mut sweep = Sweep { from_hz: None };
/// let id = SweepCommand::Start.id();
/// let given = SweepCommand::commands().def(id).unwrap().new_args();
/// let args = SweepCommand::commands().check(id, &given).unwrap();
/// sweep.invoke(id, &args).unwrap();
/// assert_eq!(sweep.from_hz, Some(88e6));
/// ```
#[macro_export]
macro_rules! commands {
    ($(#[$m:meta])* $vis:vis enum $name:ident {
        $($(#[$vm:meta])* $var:ident = $command:expr),+ $(,)?
    }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[repr(u16)]
        $vis enum $name {
            $($(#[$vm])* $var),+
        }
        impl $crate::command::CommandSet for $name {
            const COMMANDS: &'static [&'static $crate::command::Command] = &[$(&$command),+];
            fn commands() -> &'static $crate::command::Commands {
                static COMMANDS: ::std::sync::OnceLock<$crate::command::Commands> = ::std::sync::OnceLock::new();
                COMMANDS.get_or_init(|| $crate::command::Commands::of(Self::COMMANDS))
            }
            fn id(self) -> $crate::command::CommandId {
                $crate::command::CommandId(self as u16)
            }
            fn from_id(id: $crate::command::CommandId) -> Option<Self> {
                const ALL: &[$name] = &[$($name::$var),+];
                ALL.get(id.index()).copied()
            }
        }
    };
}

/// Why a command was refused or failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum CommandError {
    /// No command has this key. Only a lookup by key returns it.
    #[error("there is no command {key}")]
    Unknown {
        /// The key.
        key: String,
    },
    /// No command has this id.
    #[error("there is no command number {}", .id.0)]
    NoSuchId {
        /// The id.
        id: CommandId,
    },
    /// The argument record is not built on the argument declaration of the command.
    #[error("the arguments are not those of command number {}", .id.0)]
    ArgumentsDecl {
        /// The command.
        id: CommandId,
    },
    /// A required argument was not given.
    #[error("argument number {} is required", .arg.0)]
    MissingArgument {
        /// The argument.
        arg: SettingId,
    },
    /// The command declares no argument with this key, or declares it with another kind.
    #[error("{key} is not an argument of this command")]
    NotAnArgument {
        /// The key of the requested argument.
        key: &'static str,
    },
    /// An argument is refused.
    #[error("argument number {} is refused: {error}", .arg.0)]
    BadArgument {
        /// The argument.
        arg: SettingId,
        /// Why it is refused.
        error: SettingError,
    },
    /// The state of the owner does not allow the command now.
    #[error("not now: field number {} of the results is {state:?}", .field.0)]
    NotNow {
        /// The results field that holds the state of the owner.
        field: FieldId,
        /// The value of that field now.
        state: Value,
    },
    /// The row that the command names is not in the table.
    #[error("table number {} has no row {key}", .table.0)]
    NoSuchRow {
        /// The table.
        table: TableId,
        /// The row key given.
        key: String,
    },
    /// The owner names a table that its results do not declare. That is a fault of the owner.
    #[error("table {key} is not declared")]
    NoSuchTable {
        /// The key of the table.
        key: &'static str,
    },
    /// Argument `arg` names a thing that does not exist, and the owner has no table that lists such things.
    #[error("argument number {} names nothing that exists", .arg.0)]
    NoSuchKey {
        /// The argument.
        arg: SettingId,
    },
    /// The command transmits, and the transmitter of the session is off: the transmit switch of the engine is off, or
    /// the session has no radio that can transmit. Nothing was sent.
    #[error("transmit is off: the transmit switch of the engine is off, or the session has no radio that can transmit")]
    TransmitOff,
    /// The owner tried the command and it failed.
    #[error("the command failed: {source}")]
    Failed {
        /// The typed error of the owner.
        source: StageError,
    },
}
