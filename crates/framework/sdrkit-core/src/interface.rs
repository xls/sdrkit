//! The interface description of a stack family: struct shapes for table rows, result sets and events, typed tables and
//! event kinds, and the list of what each stack of the family declares.
//!
//! A shape is a Rust struct with `#[derive(Fields)]`. Each struct field is one declared field: its name is the key, its
//! type gives the kind, and `Option` marks a field that can be without a value. The derive makes a [`Column`] constant
//! for each field (a key, read with [`Record::at`], and no write), a writer that takes the whole struct, and a reader. A command
//! with arguments or a reply is a [`CommandKind`] over an arguments struct (`#[derive(Arguments)]`) and a reply shape.
//! A [`Family`] lists the stacks with their shapes, settings and commands. The run-time declaration, the registry
//! record, the C header and the other generated files come from it.
//!
//! ```
//! use sdrkit_core::interface::{Fields, ReadFields, Table};
//! use sdrkit_core::{Published, ResultsDecl};
//! use std::sync::Arc;
//!
//! /// A ship heard.
//! #[derive(Fields)]
//! struct ShipRow<'a> {
//!     /// The name.
//!     #[field(label = "Name")]
//!     name: &'a str,
//!     /// The speed over the ground.
//!     #[field(label = "Speed", unit = "kt")]
//!     speed_kt: Option<f64>,
//! }
//!
//! static SHIPS: Table<ShipRow<'static>> = Table::new("ships", "Ships", 300);
//!
//! let mut p = Published::from_decl(Arc::new(ResultsDecl::new().with_table(&SHIPS)));
//! SHIPS.put(&mut p, "235009000", &ShipRow { name: "PRIDE OF KENT", speed_kt: None });
//! let (key, ship) = SHIPS.rows::<ShipRow>(p.results()).next().unwrap();
//! assert_eq!((key, ship.name, ship.speed_kt), ("235009000", "PRIDE OF KENT", None));
//! ```

use crate::command::{Command, CommandError, Commands};
use crate::event::{Event, EventDef};
use crate::marks::{AudioKind, ContentKind, Media, Role, RowMarks, SortOrder, TableRole};
use crate::owner::Owner;
use crate::publish::{field, Declare, Field, Published, RasterDef, SeriesDef, TableDef};
use crate::record::{FieldDecl, Kind, Record};
use crate::results::{Results, ResultsDecl, RowMut};
use crate::settings::{DeclareSetting, Schema, Values};
use crate::value::{Choice, Value, ValueKind};
use std::marker::PhantomData;
use std::sync::Arc;

pub mod layout;

pub use sdrkit_macros::{Arguments, Fields};

/// One field of a shape, as a constant of the struct (`AircraftRow::ICAO`): its key, and with [`Record::at`] a typed
/// read of one field. It has no write. A field is written with its whole struct.
pub struct Column<K> {
    /// The key.
    pub key: &'static str,
    /// The field can be without a value.
    pub optional: bool,
    field: Field<K>,
}

impl<K> Clone for Column<K> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<K> Copy for Column<K> {}

impl<K: Kind> Column<K> {
    /// For the derive.
    #[doc(hidden)]
    pub const fn __new(field: Field<K>, optional: bool) -> Self {
        Column {
            key: field.key,
            optional,
            field,
        }
    }
    /// For the derive.
    #[doc(hidden)]
    pub const fn __field(&self) -> &Field<K> {
        &self.field
    }
}

impl Record {
    /// The value of the field of column `c`. `None` if the record does not declare the field with this kind or holds
    /// no value.
    pub fn at<K: Kind>(&self, c: &Column<K>) -> Option<K::Out<'_>> {
        self.get(&c.field)
    }
}

impl Event {
    /// The value of the field of column `c` of the event (see [`Record::at`]).
    pub fn at<K: Kind>(&self, c: &Column<K>) -> Option<K::Out<'_>> {
        self.fields.get(&c.field)
    }
}

impl<K: Kind> Declare for Column<K> {
    fn key(&self) -> &'static str {
        self.key
    }
    fn kind(&self) -> ValueKind {
        K::VALUE_KIND
    }
    fn decl(&self) -> FieldDecl {
        self.field.decl()
    }
    fn role(&self) -> Option<Role> {
        self.field.role_of()
    }
}

impl<K> std::fmt::Debug for Column<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Column").field("key", &self.key).finish()
    }
}

/// A frequency in Hz, as the type of a struct field of kind [`field::Hz`].
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Hertz(pub f64);

/// A time in seconds since 1970 (UTC), as the type of a struct field of kind [`field::Time`].
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Time(pub f64);

/// A length of time in seconds, as the type of a struct field of kind [`field::Duration`].
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

/// A Rust type that a struct field of a shape can have. The type selects the kind of the declared field.
///
/// | Type | Kind |
/// |---|---|
/// | `&str` | text |
/// | `i64` | whole number |
/// | `f64` | number |
/// | `bool` | yes or no |
/// | [`Hertz`], [`Time`], [`Seconds`] | frequency, time, length of time |
/// | `(f64, f64)` | position, latitude first |
/// | an enum of [`choice_enum!`](crate::choice_enum) | choice |
/// | `Arc<[u8]>` | bytes |
/// | `&Value` | a value that carries its own kind, for a column whose rows hold values of several kinds |
pub trait Cell<'a>: Sized {
    /// The kind of the declared field.
    type Kind: Kind;
    /// The value in the form that a write takes.
    fn give(&self) -> <Self::Kind as Kind>::In<'_>;
    /// The value from the form that a read returns.
    fn take(v: <Self::Kind as Kind>::Out<'a>) -> Self;
}

macro_rules! plain_cell {
    ($t:ty, $kind:ty) => {
        impl<'a> Cell<'a> for $t {
            type Kind = $kind;
            fn give(&self) -> $t {
                *self
            }
            fn take(v: $t) -> Self {
                v
            }
        }
    };
}

plain_cell!(i64, field::I64);
plain_cell!(f64, field::F64);
plain_cell!(bool, field::Bool);
plain_cell!((f64, f64), field::Position);

macro_rules! wrapped_cell {
    ($t:ident, $kind:ty) => {
        impl<'a> Cell<'a> for $t {
            type Kind = $kind;
            fn give(&self) -> f64 {
                self.0
            }
            fn take(v: f64) -> Self {
                $t(v)
            }
        }
    };
}

wrapped_cell!(Hertz, field::Hz);
wrapped_cell!(Time, field::Time);
wrapped_cell!(Seconds, field::Duration);

impl<'a> Cell<'a> for &'a str {
    type Kind = field::Text;
    fn give(&self) -> &str {
        self
    }
    fn take(v: &'a str) -> Self {
        v
    }
}

impl<'a, E: Choice> Cell<'a> for E {
    type Kind = field::Choice<E>;
    fn give(&self) -> E {
        *self
    }
    fn take(v: E) -> Self {
        v
    }
}

impl<'a> Cell<'a> for &'a Value {
    type Kind = field::Any;
    fn give(&self) -> &Value {
        self
    }
    fn take(v: &'a Value) -> Self {
        v
    }
}

impl<'a> Cell<'a> for Arc<[u8]> {
    type Kind = field::Bytes;
    fn give(&self) -> Arc<[u8]> {
        Arc::clone(self)
    }
    fn take(v: &'a [u8]) -> Self {
        Arc::from(v)
    }
}

/// Where a shape writes its fields: a table row, a record, or the result fields of a store.
pub trait Sink {
    /// Writes field `f`.
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>);
    /// Writes field `f` if `v` is `Some`, else marks it as having no value.
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>);
}

impl Sink for Record {
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        Record::set(self, f, v);
    }
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        Record::opt(self, f, v);
    }
}

impl Sink for RowMut<'_> {
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        RowMut::set(self, f, v);
    }
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        RowMut::opt(self, f, v);
    }
}

impl Sink for Published {
    fn set<K: Kind>(&mut self, f: &Field<K>, v: K::In<'_>) {
        Published::set(self, f, v);
    }
    fn opt<K: Kind>(&mut self, f: &Field<K>, v: Option<K::In<'_>>) {
        Published::opt(self, f, v);
    }
}

/// What the description says about one field of a shape beyond its declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldNote {
    /// The field can be without a value: the struct field is an `Option`.
    pub optional: bool,
    /// What the field holds, for people: the doc comment of the struct field.
    pub doc: &'static str,
}

/// A struct shape as data: its name, what it holds, and its fields in order.
#[derive(Clone, Copy)]
pub struct Shape {
    /// The name of the Rust struct (`AircraftRow`).
    pub name: &'static str,
    /// What the shape holds, for people: the doc comment of the struct.
    pub doc: &'static str,
    /// The declared fields, in order.
    pub fields: &'static [&'static dyn Declare],
    /// One note for each field.
    pub notes: &'static [FieldNote],
}

impl std::fmt::Debug for Shape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shape").field("name", &self.name).finish()
    }
}

/// A struct whose fields are declared fields. Implement it with `#[derive(Fields)]`, not by hand.
///
/// Attributes of a struct field, in `#[field(...)]`:
/// - `label = "..."`: what people see. Required unless `shared` is given.
/// - `unit = "..."`: the unit people see.
/// - `refers_to = "table"`: the field holds row keys of this table of the same owner.
/// - `content = PATH`: what the bytes of the field hold, a constant of the content type at `PATH`.
/// - `shared = PATH`: the field is the [`Field`] constant at `PATH` (a standard key). Its kind must be the kind of the
///   struct field's type, and its key must be the name of the struct field.
///
/// A result set that several components write has one struct for the whole set, which the description names, and one
/// struct for the part that each component writes: `#[fields(part_of = Whole)]` on the struct of the part. A field of
/// a part has the name and the type of the same field of the whole set, and no attribute. A part is written and read
/// as any shape is.
pub trait Fields: Sized {
    /// The same struct with `'static` borrows. It names the shape where no borrow is at hand.
    type Static: Fields + 'static;
    /// The shape as data.
    const SHAPE: Shape;
    /// Writes every field to `out`: a plain field with [`Sink::set`], an `Option` with [`Sink::opt`].
    fn write(&self, out: &mut impl Sink);
}

/// A shape that reads itself from a record.
pub trait ReadFields<'a>: Fields {
    /// The fields of `rec`. `None` if a field that is not an `Option` has no value in `rec`, or `rec` does not declare it.
    fn read(rec: &'a Record) -> Option<Self>;
}

/// The shape without fields: the reply of a command that has none.
impl Fields for () {
    type Static = ();
    const SHAPE: Shape = Shape {
        name: "",
        doc: "",
        fields: &[],
        notes: &[],
    };
    fn write(&self, _out: &mut impl Sink) {}
}

impl ReadFields<'_> for () {
    fn read(_rec: &Record) -> Option<Self> {
        Some(())
    }
}

/// The arguments of a command as data: the name of the struct and its arguments in order.
#[derive(Clone, Copy)]
pub struct ArgShape {
    /// The name of the Rust struct (`RemoveArgs`). Empty for a command without arguments.
    pub name: &'static str,
    /// What the struct holds, for people.
    pub doc: &'static str,
    /// The declared arguments, in order.
    pub args: &'static [&'static dyn DeclareSetting],
    /// What each argument holds, for people: the doc comment of the struct field.
    pub docs: &'static [&'static str],
}

impl std::fmt::Debug for ArgShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArgShape")
            .field("name", &self.name)
            .finish()
    }
}

/// A struct whose fields are the arguments of a command. Implement it with `#[derive(Arguments)]`, not by hand.
///
/// A struct field of a plain type is a required argument. A struct field of type `Option` is an optional argument: it
/// has a default, and a caller that passes `None` leaves it out. The owner that runs the command always reads `Some`,
/// with the given value or the default.
///
/// Attributes of a struct field, in `#[arg(...)]`:
/// - `label = "..."`: what people see. Required unless `shared` is given.
/// - `unit = "..."`, `help = "..."`: the unit and one or two sentences for people.
/// - `refers_to = "table"`: the argument holds a row key of this table of the same owner.
/// - `default = VALUE`: the value in force when an optional argument is left out. Required for an `Option` unless
///   `shared` is given.
/// - `min = N, max = N, step = N`: the limits of an optional number argument, all three together.
/// - `shared = PATH`: the argument is the constant at `PATH`: an [`Argument`](crate::settings::Argument) for a
///   required argument, a [`Setting`](crate::settings::Setting) for an optional one. Its key must be the name of the
///   struct field.
///
/// ```
/// use sdrkit_core::interface::{Arguments, CommandKind};
///
/// /// The arguments of `send`.
/// #[derive(Arguments)]
/// struct SendArgs<'a> {
///     /// The text.
///     #[arg(label = "Text")]
///     text: &'a str,
///     /// The addressee.
///     #[arg(label = "To", default = "^all")]
///     to: Option<&'a str>,
///     /// The packet that the text replies to.
///     #[arg(label = "Reply to", default = 0, min = 0, max = 4_294_967_295.0, step = 1)]
///     reply_id: Option<i64>,
/// }
///
/// const SEND: CommandKind<SendArgs<'static>> = CommandKind::new("send", "Send");
/// let def = SEND.command.def();
/// let required: Vec<bool> = def.args.settings().iter().map(|a| a.required).collect();
/// assert_eq!(required, [true, false, false]);
/// ```
pub trait Arguments: Sized {
    /// The same struct with `'static` borrows.
    type Static: Arguments + 'static;
    /// The arguments as data.
    const SHAPE: ArgShape;
    /// Writes every argument to `rec`, an argument record of the command ([`CommandDef::new_args`](crate::CommandDef::new_args)).
    /// An optional argument that is `None` is not written.
    fn write(&self, rec: &mut Record);
}

/// An arguments struct that reads itself from the checked arguments of a call.
pub trait ReadArguments<'a>: Arguments {
    /// The arguments in `values`.
    ///
    /// # Errors
    ///
    /// What [`Values::arg`] returns, if `values` is not of the command of these arguments.
    fn read(values: &'a Values) -> Result<Self, CommandError>;
}

/// No arguments.
impl Arguments for () {
    type Static = ();
    const SHAPE: ArgShape = ArgShape {
        name: "",
        doc: "",
        args: &[],
        docs: &[],
    };
    fn write(&self, _rec: &mut Record) {}
}

impl ReadArguments<'_> for () {
    fn read(_values: &Values) -> Result<Self, CommandError> {
        Ok(())
    }
}

/// A command whose arguments are the struct `A` and whose reply has the shape `R`. Name it once as a `const`, and list
/// its `command` in the `commands!` enum of the owner.
///
/// ```
/// use sdrkit_core::interface::{Arguments, CommandKind, Fields};
///
/// /// The arguments of `rename`.
/// #[derive(Arguments)]
/// struct RenameArgs<'a> {
///     /// The row key of the ship.
///     #[arg(label = "Row", refers_to = "ships")]
///     row: &'a str,
///     /// The new name.
///     #[arg(label = "Name")]
///     name: &'a str,
/// }
///
/// /// The reply of `rename`.
/// #[derive(Fields)]
/// struct Renamed {
///     /// The number of rows that changed.
///     #[field(label = "Changed")]
///     changed: i64,
/// }
///
/// const RENAME: CommandKind<RenameArgs<'static>, Renamed> = CommandKind::new("rename", "Rename");
/// let def = RENAME.command.def();
/// assert_eq!((def.args.len(), def.reply.len()), (2, 1));
/// ```
pub struct CommandKind<A, R = ()> {
    /// The declaration.
    pub command: Command,
    shape: PhantomData<fn(A) -> R>,
}

impl<A, R> Clone for CommandKind<A, R> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A, R> Copy for CommandKind<A, R> {}

impl<A: Arguments + 'static, R: Fields + 'static> CommandKind<A, R> {
    /// A command `key` shown as `label`, with the arguments of `A` and the reply fields of `R`.
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        CommandKind {
            command: Command::new(key, label)
                .args(A::SHAPE.args)
                .reply(R::SHAPE.fields),
            shape: PhantomData,
        }
    }
    /// The same command with this help text.
    #[must_use]
    pub const fn help(mut self, help: &'static str) -> Self {
        self.command = self.command.help(help);
        self
    }
    /// The same command, declared as one that transmits `content` ([`Command::transmits`]).
    #[must_use]
    pub const fn transmits(mut self, content: crate::TxContent) -> Self {
        self.command = self.command.transmits(content);
        self
    }
    /// The same command, whose transmission lasts until the command `key` stops it ([`Command::stopped_by`]).
    #[must_use]
    pub const fn stopped_by(mut self, key: &'static str) -> Self {
        self.command = self.command.stopped_by(key);
        self
    }
    /// The arguments of a call, for the owner that runs the command.
    ///
    /// # Errors
    ///
    /// What [`ReadArguments::read`] returns.
    pub fn args<'a, X: ReadArguments<'a, Static = A>>(
        &self,
        values: &'a Values,
    ) -> Result<X, CommandError> {
        X::read(values)
    }
    /// The reply record with the fields of `reply`, for the owner that runs the command. `commands` is the owner's list.
    /// `None` if the list does not hold the command.
    pub fn reply<X: Fields<Static = R>>(&self, commands: &Commands, reply: &X) -> Option<Record> {
        let id = commands.id(self.command.key).ok()?;
        let mut rec = commands.def(id)?.new_reply();
        reply.write(&mut rec);
        Some(rec)
    }
    /// Runs the command on `owner` with `args`, after the check of the arguments, and returns the reply record.
    ///
    /// # Errors
    ///
    /// [`CommandError::Unknown`] if the owner does not have the command, else what [`crate::owner::invoke`] returns.
    pub fn call<O: Owner + ?Sized, X: Arguments<Static = A>>(
        &self,
        owner: &mut O,
        args: &X,
    ) -> Result<Option<Record>, CommandError> {
        let id = owner.commands().id(self.command.key)?;
        let mut rec = match owner.commands().def(id) {
            Some(def) => def.new_args(),
            None => return Err(CommandError::NoSuchId { id }),
        };
        args.write(&mut rec);
        crate::owner::invoke(owner, id, &rec)
    }
    /// The reply of a call from its record. `None` if a field that is not an `Option` has no value.
    pub fn read_reply<'a, X: ReadFields<'a, Static = R>>(&self, reply: &'a Record) -> Option<X> {
        X::read(reply)
    }
}

impl<A, R> std::fmt::Debug for CommandKind<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.command.fmt(f)
    }
}

/// A command of a stack: its declaration, the struct of its arguments and the shape of its reply.
#[derive(Clone, Copy, Debug)]
pub struct CommandShape {
    /// The declaration.
    pub command: &'static Command,
    /// The arguments.
    pub args: ArgShape,
    /// The reply.
    pub reply: Shape,
}

/// A command without arguments and without a reply.
///
/// # Panics
///
/// If the command declares an argument or a reply field: describe it as a [`CommandKind`].
impl From<&'static Command> for CommandShape {
    fn from(command: &'static Command) -> Self {
        assert!(
            command.args.is_empty() && command.reply.is_empty(),
            "the command {} has arguments or a reply: describe it as a CommandKind",
            command.key
        );
        CommandShape {
            command,
            args: <() as Arguments>::SHAPE,
            reply: <() as Fields>::SHAPE,
        }
    }
}

impl<A: Arguments + 'static, R: Fields + 'static> From<&'static CommandKind<A, R>>
    for CommandShape
{
    fn from(kind: &'static CommandKind<A, R>) -> Self {
        CommandShape {
            command: &kind.command,
            args: A::SHAPE,
            reply: R::SHAPE,
        }
    }
}

impl Published {
    /// Writes the result fields of `set`. The declaration of the store must hold them ([`ResultsDecl::with_fields`] of
    /// the shape's fields).
    pub fn put<F: Fields>(&mut self, set: &F) {
        set.write(self);
    }
}

impl Event {
    /// The same event with the fields of `fields` written. [`EventKind::event`] is the call for a stack.
    #[must_use]
    fn put<F: Fields>(mut self, fields: &F) -> Self {
        fields.write(&mut self.fields);
        self
    }
}

/// A table whose rows have the shape `S`. It is a [`TableDef`] for the calls that take one.
pub struct Table<S> {
    def: TableDef,
    row: PhantomData<fn() -> S>,
}

impl<S: Fields + 'static> Table<S> {
    /// A table `key` shown as `label` that keeps at most `cap` rows of shape `S`.
    pub const fn new(key: &'static str, label: &'static str, cap: usize) -> Self {
        Table {
            def: TableDef {
                key,
                label,
                columns: S::SHAPE.fields,
                cap,
                marks: RowMarks::NONE,
            },
            row: PhantomData,
        }
    }
    /// The same table, in which column `c` names a row for people: the call sign, the name of the station, the id of
    /// the device. `c` is a column constant of the row shape (`ShipRow::NAME`).
    #[must_use]
    pub const fn label<K>(mut self, c: &Column<K>) -> Self {
        self.def.marks.label = c.key;
        self
    }
    /// The same table, whose rows a front end shows in the order `order` of column `c` unless the user chooses
    /// another order. A table that states no order shows its newest row first. The order of the rows in a snapshot
    /// does not change.
    #[must_use]
    pub const fn sorted<K>(mut self, c: &Column<K>, order: SortOrder) -> Self {
        self.def.marks.sort = c.key;
        self.def.marks.order = order;
        self
    }
    /// The same table, whose rows are what `role` says: the messages, the calls, the nodes or the groups of the
    /// stack. A front end finds the table by its role and not by its key. The columns of a message have their own
    /// roles (`#[field(.., role = Sender)]`, `Addressee`, `Text`, `Time`).
    #[must_use]
    pub const fn role(mut self, role: TableRole) -> Self {
        self.def.marks.role = Some(role);
        self
    }
    /// The declaration of the table.
    pub const fn def(&self) -> &TableDef {
        &self.def
    }
    /// The declaration of a table `key` shown as `label` that keeps at most `cap` rows of shape `S`, for a constant of
    /// a shared crate that a stack lists by declaration.
    pub const fn def_of(key: &'static str, label: &'static str, cap: usize) -> TableDef {
        TableDef {
            key,
            label,
            columns: S::SHAPE.fields,
            cap,
            marks: RowMarks::NONE,
        }
    }
    /// The shape of the rows.
    pub const fn row(&self) -> Shape {
        S::SHAPE
    }
    /// Writes row `key` of this table in `p` from `row`, and makes it the newest row. Only the cells that changed are
    /// written. The declaration of `p` must hold the table.
    pub fn put<R: Fields<Static = S>>(&self, p: &mut Published, key: &str, row: &R) {
        if let Some(mut out) = p.row_mut(&self.def, key) {
            row.write(&mut out);
        }
    }
    /// The rows of this table in `results`, newest first, each with its key. A row that does not read is left out. The
    /// iterator is empty if `results` does not declare the table.
    pub fn rows<'a, R: ReadFields<'a, Static = S>>(
        &self,
        results: &'a Results,
    ) -> impl Iterator<Item = (&'a str, R)> {
        self.rows_at(self.def.key, results)
    }
    /// As [`Table::rows`], for the table of the component `component` in the results of a stack that lists it with
    /// [`StackShape::component`].
    pub fn rows_of<'a, R: ReadFields<'a, Static = S>>(
        &self,
        component: &str,
        results: &'a Results,
    ) -> impl Iterator<Item = (&'a str, R)> {
        self.rows_at(&format!("{component}.{}", self.def.key), results)
    }
    fn rows_at<'a, R: ReadFields<'a, Static = S>>(
        &self,
        key: &str,
        results: &'a Results,
    ) -> impl Iterator<Item = (&'a str, R)> {
        results
            .decl()
            .table_id(key)
            .and_then(|id| results.table(id))
            .into_iter()
            .flat_map(|t| t.rows_newest_first())
            .filter_map(|(k, r)| Some((k, R::read(r)?)))
    }
}

impl<S> std::ops::Deref for Table<S> {
    type Target = TableDef;
    fn deref(&self) -> &TableDef {
        &self.def
    }
}

impl<S> std::fmt::Debug for Table<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.def.fmt(f)
    }
}

/// An event kind whose events have the fields of the shape `S`. It is an [`EventDef`] for the calls that take one.
pub struct EventKind<S> {
    def: EventDef,
    fields: PhantomData<fn() -> S>,
}

impl<S: Fields + 'static> EventKind<S> {
    /// An event kind `kind` shown as `label`, with the fields of shape `S`.
    pub const fn new(kind: &'static str, label: &'static str) -> Self {
        EventKind {
            def: EventDef::new(kind, label, S::SHAPE.fields),
            fields: PhantomData,
        }
    }
    /// The declaration of the kind.
    pub const fn def(&self) -> &EventDef {
        &self.def
    }
    /// The shape of the fields.
    pub const fn shape(&self) -> Shape {
        S::SHAPE
    }
    /// A new event of this kind at signal time `time` on `freq_hz` with the fields of `fields`.
    pub fn event<R: Fields<Static = S>>(
        &'static self,
        time: f64,
        freq_hz: f64,
        fields: &R,
    ) -> Event {
        self.def.event(time, freq_hz).put(fields)
    }
    /// The fields of `event`. `None` if the event is of another kind or a field that is not an `Option` has no value.
    pub fn read<'a, R: ReadFields<'a, Static = S>>(&self, event: &'a Event) -> Option<R> {
        event.is(&self.def).then(|| R::read(&event.fields))?
    }
}

impl<S> std::ops::Deref for EventKind<S> {
    type Target = EventDef;
    fn deref(&self) -> &EventDef {
        &self.def
    }
}

impl<S> std::fmt::Debug for EventKind<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.def.fmt(f)
    }
}

/// The version of a family's interface. An addition raises `minor`. A removal, a rename or a change of a type or a unit
/// raises `major`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Raised by a change that an existing consumer does not survive.
    pub major: u16,
    /// Raised by an addition.
    pub minor: u16,
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A table of a stack: its declaration and the shape of its rows.
#[derive(Clone, Copy, Debug)]
pub struct TableShape {
    /// The declaration.
    pub def: &'static TableDef,
    /// The shape of a row.
    pub row: Shape,
}

/// An event kind of a stack: its declaration and the shape of its fields.
#[derive(Clone, Copy, Debug)]
pub struct EventShape {
    /// The declaration.
    pub def: &'static EventDef,
    /// The shape of the fields.
    pub fields: Shape,
}

impl<S: Fields + 'static> From<&'static EventKind<S>> for EventShape {
    fn from(e: &'static EventKind<S>) -> Self {
        EventShape {
            def: &e.def,
            fields: S::SHAPE,
        }
    }
}

impl EventShape {
    /// The event type `def`, whose declaration is made from the fields of the shape `S`
    /// (`EventDef::new(kind, label, S::SHAPE.fields)`). It is for a crate whose components return events through a
    /// declaration, not through an [`EventKind`].
    ///
    /// # Panics
    ///
    /// If the fields of `def` are not the fields of `S`.
    pub fn of<S: Fields>(def: &'static EventDef) -> Self {
        let keys = |fields: &[&dyn Declare]| -> Vec<&'static str> {
            fields.iter().map(|f| f.key()).collect()
        };
        assert!(
            keys(def.fields) == keys(S::SHAPE.fields),
            "the event type {} does not have the fields of {}",
            def.kind,
            S::SHAPE.name
        );
        EventShape {
            def,
            fields: S::SHAPE,
        }
    }
}

/// What one stack declares: result sets, tables, series, map points, event kinds, settings and commands.
#[derive(Clone)]
pub struct StackShape {
    /// The stack id.
    pub id: &'static str,
    /// The result sets, in order. The result fields of the stack are their fields, one set after the other.
    pub fields: Vec<Shape>,
    /// The tables, in order.
    pub tables: Vec<TableShape>,
    /// The series, in order.
    pub series: Vec<SeriesDef>,
    /// The rasters, in order.
    pub rasters: Vec<RasterDef>,
    /// The label of the map points. Empty for none.
    pub points: &'static str,
    /// The event kinds, in order.
    pub events: Vec<EventShape>,
    /// The settings, in order.
    pub settings: Vec<&'static dyn DeclareSetting>,
    /// The commands, in order.
    pub commands: Vec<CommandShape>,
    /// The result sets of named components, in order. Their fields follow those of `fields` in the results of the
    /// stack, each with the key `<component>.<key>`.
    pub components: Vec<ComponentFields>,
    /// What survives a change of frequency by the host ([`StackShape::kept`]): the key of a table, the name of the
    /// struct of a result set, or `<Struct>.<key>` for one field of a result set.
    pub kept: Vec<&'static str>,
    /// The names of the structs in `fields` that the receiver of the stack writes itself, not a component
    /// ([`StackShape::receiver_fields`]).
    pub receiver_sets: Vec<&'static str>,
    /// The tables of the stack that named components write, each with the names of its writers
    /// ([`ComponentShape::writes_table`]).
    pub table_writers: Vec<(&'static str, Vec<&'static str>)>,
    /// The names of the structs in `fields` whose fields follow those of the components in the results of the stack
    /// ([`StackShape::fields_after_components`]).
    pub late_sets: Vec<&'static str>,
    /// The kind of content of the stack ([`StackShape::content`]). `None` for a description of an object or of a
    /// shared interface.
    pub content: Option<ContentKind>,
    /// The media that the stack states ([`StackShape::audio`], [`StackShape::pictures`], [`StackShape::video`],
    /// [`StackShape::text`]). [`StackShape::media`] returns these and the media that follow from the declarations.
    pub stated_media: Media,
}

/// A text of a description. A description is made once and lives as long as the program.
fn keep(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// A declared field under the key `<component>.<key>`.
struct Prefixed {
    key: &'static str,
    of: &'static dyn Declare,
}

impl Declare for Prefixed {
    fn key(&self) -> &'static str {
        self.key
    }
    fn kind(&self) -> ValueKind {
        self.of.kind()
    }
    fn decl(&self) -> FieldDecl {
        let mut d = self.of.decl();
        d.key = std::borrow::Cow::Borrowed(self.key);
        d
    }
    fn role(&self) -> Option<Role> {
        self.of.role()
    }
}

/// The count of dropped items of a decoder component, as every decoder component declares it.
static DROPPED: Field<field::I64> = crate::publish::keys::DROPPED;

/// One result set of a named component of a stack. A generic consumer reads its fields under the keys
/// `<component>.<key>`.
#[derive(Clone, Copy)]
pub struct ComponentFields {
    /// The name of the component in the graph of the stack.
    pub component: &'static str,
    /// The shape of the set. `None` for the count of dropped items of a decoder component, the one field
    /// `<component>.dropped`.
    pub shape: Option<Shape>,
    /// The declared fields with their keys in the results of the stack, in the order of the shape, without the
    /// fields of `shared`.
    pub fields: &'static [&'static dyn Declare],
    /// The keys of the fields of the shape that are not `<component>.<key>` in the results of the stack: the
    /// component writes the field `<key>` that a result set of the stack declares ([`ComponentShape::shares`]).
    pub shared: &'static [&'static str],
}

impl std::fmt::Debug for ComponentFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentFields")
            .field("component", &self.component)
            .field("shape", &self.shape.map(|s| s.name))
            .finish()
    }
}

impl ComponentFields {
    fn new(component: &'static str, shape: Option<Shape>, shared: &'static [&'static str]) -> Self {
        let own: &[&'static dyn Declare] = match &shape {
            Some(s) => s.fields,
            None => &[&DROPPED],
        };
        let fields: Vec<&'static dyn Declare> = own
            .iter()
            .filter(|f| !shared.contains(&f.key()))
            .map(|f| {
                let p: &'static dyn Declare = Box::leak(Box::new(Prefixed {
                    key: keep(format!("{component}.{}", f.key())),
                    of: *f,
                }));
                p
            })
            .collect();
        ComponentFields {
            component,
            shape,
            fields: Box::leak(fields.into_boxed_slice()),
            shared,
        }
    }
    /// The key in the results of the stack of the field `key` of the shape: `<component>.<key>`, or `key` for a
    /// field of `shared`.
    pub fn shown(&self, key: &str) -> String {
        if self.shared.contains(&key) {
            key.to_owned()
        } else {
            format!("{}.{key}", self.component)
        }
    }
}

/// What one named component of a stack adds to the results of the stack, for [`StackShape::component`].
#[derive(Debug)]
pub struct ComponentShape {
    name: &'static str,
    stack: StackShape,
}

impl ComponentShape {
    /// The same component with the result set `F`: the fields of the stack `<component>.<key>`.
    #[must_use]
    pub fn fields<F: Fields>(mut self) -> Self {
        self.stack
            .components
            .push(ComponentFields::new(self.name, Some(F::SHAPE), &[]));
        self
    }
    /// The same component, in which the fields `keys` of the result set listed last are not `<component>.<key>`:
    /// the component writes the field `<key>` of the stack, which a result set of the stack declares. Several
    /// components can share one key. The stack shows the value of the status component if it holds one, else of the
    /// first writer in run order that holds one.
    ///
    /// ```
    /// use sdrkit_core::interface::{Fields, StackShape};
    ///
    /// /// The totals of the stack.
    /// #[derive(Fields)]
    /// struct RelayFields {
    ///     /// The good frames of every decoder.
    ///     #[field(label = "Frames good")]
    ///     frames_ok: i64,
    /// }
    /// /// The counts of one decoder.
    /// #[derive(Fields)]
    /// struct LinkFields {
    ///     /// The good frames.
    ///     #[field(label = "Frames good")]
    ///     frames_ok: i64,
    ///     /// The bad frames.
    ///     #[field(label = "Frames bad")]
    ///     frames_bad: i64,
    /// }
    ///
    /// let s = StackShape::new("relay")
    ///     .fields::<RelayFields>()
    ///     .component("link", |c| c.fields::<LinkFields>().shares(&["frames_ok"]));
    /// let d = s.results_decl();
    /// let keys: Vec<&str> = d.fields().fields().iter().map(|f| f.key.as_ref()).collect();
    /// assert_eq!(keys, ["frames_ok", "link.frames_bad"]);
    /// ```
    ///
    /// # Panics
    ///
    /// If the component has no result set yet, or its last set has no field of a key.
    #[must_use]
    pub fn shares(mut self, keys: &'static [&'static str]) -> Self {
        let last = self
            .stack
            .components
            .last_mut()
            .filter(|c| c.component == self.name && c.shape.is_some())
            .unwrap_or_else(|| {
                panic!(
                    "the component {} has no result set to share from",
                    self.name
                )
            });
        let shape = last.shape.expect("a set with a shape");
        for k in keys {
            assert!(
                shape.fields.iter().any(|f| f.key() == *k),
                "{} has no field {k}",
                shape.name
            );
        }
        *last = ComponentFields::new(self.name, Some(shape), keys);
        self
    }
    /// The same component, in which every field of the result set listed last whose key a result set of the stack
    /// declares is shared ([`ComponentShape::shares`]). A shared crate that adds its component to the descriptions of
    /// several stacks calls it: each stack then decides with its own result sets which fields it shows under its own
    /// keys. List the result sets of the stack before the component.
    ///
    /// # Panics
    ///
    /// If the component has no result set yet.
    #[must_use]
    pub fn shares_declared(self) -> Self {
        let shape = self
            .stack
            .components
            .last()
            .filter(|c| c.component == self.name)
            .and_then(|c| c.shape)
            .unwrap_or_else(|| {
                panic!(
                    "the component {} has no result set to share from",
                    self.name
                )
            });
        let declared = |key: &str| {
            self.stack
                .fields
                .iter()
                .any(|s| s.fields.iter().any(|f| f.key() == key))
        };
        let keys: Vec<&'static str> = shape
            .fields
            .iter()
            .map(|f| f.key())
            .filter(|k| declared(k))
            .collect();
        self.shares(Box::leak(keys.into_boxed_slice()))
    }
    /// The same component as a writer of the table `t` of the stack, which the stack lists with
    /// [`StackShape::table`] under its own key. Name each component that writes the table. The stack shows the rows of
    /// the status component if it holds rows, else of the first writer in run order that holds rows.
    ///
    /// # Panics
    ///
    /// If the stack does not list `t` yet, or lists a table of this key with another row shape.
    #[must_use]
    pub fn writes_table<S: Fields + 'static>(mut self, t: &'static Table<S>) -> Self {
        let listed = self.stack.tables.iter().find(|x| x.def.key == t.def.key);
        assert!(
            listed.is_some_and(|x| x.row.name == S::SHAPE.name),
            "the stack {} does not list the table {} with rows of {}",
            self.stack.id,
            t.def.key,
            S::SHAPE.name
        );
        match self
            .stack
            .table_writers
            .iter_mut()
            .find(|(k, _)| *k == t.def.key)
        {
            Some((_, w)) => w.push(self.name),
            None => self.stack.table_writers.push((t.def.key, vec![self.name])),
        }
        self
    }
    /// The same component with the count of dropped items of a decoder component: the field `<component>.dropped`
    /// of the stack. The component writes it itself.
    #[must_use]
    pub fn dropped(mut self) -> Self {
        self.stack
            .components
            .push(ComponentFields::new(self.name, None, &[]));
        self
    }
    /// The same component with its table `t`: the table `<component>.<key>` of the stack.
    #[must_use]
    pub fn table<S: Fields + 'static>(mut self, t: &'static Table<S>) -> Self {
        let def: &'static TableDef = Box::leak(Box::new(TableDef {
            key: keep(format!("{}.{}", self.name, t.def.key)),
            ..t.def
        }));
        self.stack.tables.push(TableShape { def, row: S::SHAPE });
        self
    }
    /// The same component with its series `s`: the series `<component>.<key>` of the stack.
    #[must_use]
    pub fn series(mut self, s: SeriesDef) -> Self {
        self.stack.series.push(SeriesDef {
            key: keep(format!("{}.{}", self.name, s.key)),
            ..s
        });
        self
    }
}

impl StackShape {
    /// A stack `id` that declares nothing.
    pub fn new(id: &'static str) -> Self {
        StackShape {
            id,
            fields: Vec::new(),
            tables: Vec::new(),
            series: Vec::new(),
            rasters: Vec::new(),
            points: "",
            events: Vec::new(),
            settings: Vec::new(),
            commands: Vec::new(),
            components: Vec::new(),
            kept: Vec::new(),
            receiver_sets: Vec::new(),
            table_writers: Vec::new(),
            late_sets: Vec::new(),
            content: None,
            stated_media: Media {
                audio: None,
                pictures: false,
                video: false,
                text: false,
            },
        }
    }
    /// The same stack with its kind of content.
    #[must_use]
    pub fn content(mut self, kind: ContentKind) -> Self {
        self.content = Some(kind);
        self
    }
    /// The same stack, which gives sound of kind `kind`.
    #[must_use]
    pub fn audio(mut self, kind: AudioKind) -> Self {
        self.stated_media.audio = Some(kind);
        self
    }
    /// The same stack, which gives still pictures. A stack that declares a field, a column or an event field with the
    /// content of a picture needs no call: [`StackShape::media`] finds it.
    #[must_use]
    pub fn pictures(mut self) -> Self {
        self.stated_media.pictures = true;
        self
    }
    /// The same stack, which gives moving pictures.
    #[must_use]
    pub fn video(mut self) -> Self {
        self.stated_media.video = true;
        self
    }
    /// The same stack, which gives running text for people: decoded characters or whole messages.
    #[must_use]
    pub fn text(mut self) -> Self {
        self.stated_media.text = true;
        self
    }
    /// The media of the stack: what it states, and still pictures if a result field, a table column or an event field
    /// of the stack holds bytes with the content of a picture.
    pub fn media(&self) -> Media {
        let picture = |f: &&dyn Declare| {
            f.kind() == ValueKind::Bytes
                && matches!(
                    f.decl().content,
                    Some(crate::ContentType::Media(c))
                        if c.stream_kind() == Some(crate::StreamKind::Picture)
                )
        };
        let declared = self
            .fields
            .iter()
            .flat_map(|s| s.fields)
            .chain(self.components.iter().flat_map(|c| c.fields))
            .chain(self.tables.iter().flat_map(|t| t.def.columns))
            .chain(self.events.iter().flat_map(|e| e.fields.fields))
            .any(picture);
        Media {
            pictures: self.stated_media.pictures || declared,
            ..self.stated_media
        }
    }
    /// The same stack with the results of its component `name` that do not have their own keys in the stack: the
    /// component is not the status component of the graph, and the stack does not list the items itself. `add` lists
    /// them. Each has the key `<name>.<key>` in the results of the stack.
    ///
    /// ```
    /// use sdrkit_core::interface::{Fields, StackShape, Table};
    ///
    /// /// The counts of the message decoder.
    /// #[derive(Fields)]
    /// struct MessageFields {
    ///     /// The messages read.
    ///     #[field(label = "Messages")]
    ///     messages: i64,
    /// }
    /// /// A message.
    /// #[derive(Fields)]
    /// struct MessageRow<'a> {
    ///     /// The text.
    ///     #[field(label = "Text")]
    ///     text: &'a str,
    /// }
    /// static LOG: Table<MessageRow<'static>> = Table::new("log", "Messages", 100);
    ///
    /// let s = StackShape::new("relay")
    ///     .component("egc", |c| c.fields::<MessageFields>().dropped().table(&LOG))
    ///     .decoder("rx");
    /// let d = s.results_decl();
    /// let keys: Vec<&str> = d.fields().fields().iter().map(|f| f.key.as_ref()).collect();
    /// assert_eq!(keys, ["egc.messages", "egc.dropped", "rx.dropped"]);
    /// assert!(d.table_id("egc.log").is_some());
    /// ```
    #[must_use]
    pub fn component(
        self,
        name: &'static str,
        add: impl FnOnce(ComponentShape) -> ComponentShape,
    ) -> Self {
        add(ComponentShape { name, stack: self }).stack
    }
    /// The same stack with the decoder component `name`, which is not the status component of the graph: the count of
    /// the items that it dropped is the field `<name>.dropped` of the stack. The generated forms hold the counts of
    /// all such decoders of a stack in one struct.
    #[must_use]
    pub fn decoder(self, name: &'static str) -> Self {
        self.component(name, ComponentShape::dropped)
    }
    /// The names of the decoder components whose count of dropped items the stack shows ([`StackShape::decoder`],
    /// [`ComponentShape::dropped`]), in order.
    pub fn decoders(&self) -> Vec<&'static str> {
        self.components
            .iter()
            .filter(|c| c.shape.is_none())
            .map(|c| c.component)
            .collect()
    }
    /// The same stack, in which the items `keys` survive a change of frequency by the host. An entry is the key of a
    /// table, the name of the struct of a result set (every field of the set), or `<Struct>.<key>` for one field of
    /// a result set. Every other result of the stack is cleared, also in a component that holds a kept item (see
    /// [`Receiver::forget`](crate::Receiver::forget)). Name what belongs to no single frequency: a scan list, a
    /// database of stations, a log of messages.
    ///
    /// ```
    /// use sdrkit_core::interface::{Fields, StackShape, Table};
    ///
    /// /// The counts of the receiver.
    /// #[derive(Fields)]
    /// struct RelayFields {
    ///     /// The messages read since the start.
    ///     #[field(label = "Messages")]
    ///     messages: i64,
    ///     /// The level of the present signal.
    ///     #[field(label = "Level", unit = "dB")]
    ///     level_db: Option<f64>,
    /// }
    /// /// A message.
    /// #[derive(Fields)]
    /// struct MessageRow<'a> {
    ///     /// The text.
    ///     #[field(label = "Text")]
    ///     text: &'a str,
    /// }
    /// static LOG: Table<MessageRow<'static>> = Table::new("log", "Messages", 100);
    ///
    /// let s = StackShape::new("relay")
    ///     .fields::<RelayFields>()
    ///     .table(&LOG)
    ///     .kept(&["log", "RelayFields.messages"]);
    /// assert_eq!(s.kept_keys().unwrap(), ["table log", "messages"]);
    /// ```
    #[must_use]
    pub fn kept(mut self, keys: &[&'static str]) -> Self {
        self.kept.extend_from_slice(keys);
        self
    }
    /// The keys of the result fields and of the tables that survive a change of frequency by the host, as the stack
    /// shows them, the tables with `table` and a space before the key.
    ///
    /// # Errors
    ///
    /// The first entry of [`StackShape::kept`] that names no table, no result set and no field of a result set of
    /// the stack.
    pub fn kept_keys(&self) -> Result<Vec<String>, &'static str> {
        let mut out = Vec::new();
        for k in &self.kept {
            let before = out.len();
            // `<Struct>.<key>` names one field. A table key of a component has a dot too, so the table is tried too.
            let (name, one) = match k.split_once('.') {
                Some((name, key)) => (name, Some(key)),
                None => (*k, None),
            };
            let wanted = |key: &str| one.is_none_or(|o| o == key);
            for set in self.fields.iter().filter(|f| f.name == name) {
                out.extend(
                    set.fields
                        .iter()
                        .filter(|f| wanted(f.key()))
                        .map(|f| f.key().to_owned()),
                );
            }
            for c in &self.components {
                let Some(set) = c.shape.filter(|s| s.name == name) else {
                    continue;
                };
                out.extend(
                    set.fields
                        .iter()
                        .filter(|f| wanted(f.key()))
                        .map(|f| c.shown(f.key())),
                );
            }
            if self.tables.iter().any(|t| t.def.key == *k) {
                out.push(format!("table {k}"));
            }
            if out.len() == before {
                return Err(k);
            }
        }
        out.dedup();
        Ok(out)
    }
    /// The same stack with the result set `F`, which the receiver of the stack writes itself and no component
    /// declares. A receiver that is a graph writes the count of the input samples that its guard replaced
    /// (`sdrkit_graph::GuardFields`). The set has its place among the result sets by the order of the calls.
    #[must_use]
    pub fn receiver_fields<F: Fields>(mut self) -> Self {
        self.fields.push(F::SHAPE);
        self.receiver_sets.push(F::SHAPE.name);
        self
    }
    /// The keys of the fields that the receiver of the stack writes itself ([`StackShape::receiver_fields`]).
    pub fn receiver_keys(&self) -> Vec<&'static str> {
        self.fields
            .iter()
            .filter(|f| self.receiver_sets.contains(&f.name))
            .flat_map(|f| f.fields.iter().map(|d| d.key()))
            .collect()
    }
    /// The same stack with the result set `F` added.
    #[must_use]
    pub fn fields<F: Fields>(mut self) -> Self {
        self.fields.push(F::SHAPE);
        self
    }
    /// The same stack with the result set `F` added, whose fields follow the fields of the components in the results
    /// of the stack. A receiver that wraps a graph and adds fields of its own after the results of the graph lists
    /// them so. In every other respect the set is a result set of the stack.
    ///
    /// ```
    /// use sdrkit_core::interface::{Fields, StackShape};
    ///
    /// /// The counts of the stack.
    /// #[derive(Fields)]
    /// struct RelayFields {
    ///     /// The good frames.
    ///     #[field(label = "Frames good")]
    ///     frames_ok: i64,
    /// }
    /// /// The counts of the decoder.
    /// #[derive(Fields)]
    /// struct LinkFields {
    ///     /// The bad frames.
    ///     #[field(label = "Frames bad")]
    ///     frames_bad: i64,
    /// }
    /// /// What the receiver adds.
    /// #[derive(Fields)]
    /// struct ChoiceFields {
    ///     /// The decoder that runs now.
    ///     #[field(label = "Decoder")]
    ///     decoder: i64,
    /// }
    ///
    /// let s = StackShape::new("relay")
    ///     .fields::<RelayFields>()
    ///     .component("link", |c| c.fields::<LinkFields>())
    ///     .fields_after_components::<ChoiceFields>();
    /// let d = s.results_decl();
    /// let keys: Vec<&str> = d.fields().fields().iter().map(|f| f.key.as_ref()).collect();
    /// assert_eq!(keys, ["frames_ok", "link.frames_bad", "decoder"]);
    /// ```
    #[must_use]
    pub fn fields_after_components<F: Fields>(mut self) -> Self {
        self.fields.push(F::SHAPE);
        self.late_sets.push(F::SHAPE.name);
        self
    }
    /// The same stack with table `t` added.
    #[must_use]
    pub fn table<S: Fields + 'static>(mut self, t: &'static Table<S>) -> Self {
        self.tables.push(TableShape {
            def: &t.def,
            row: S::SHAPE,
        });
        self
    }
    /// The same stack with series `s` added.
    #[must_use]
    pub fn series(mut self, s: SeriesDef) -> Self {
        self.series.push(s);
        self
    }
    /// The same stack with raster `r` added.
    #[must_use]
    pub fn raster(mut self, r: RasterDef) -> Self {
        self.rasters.push(r);
        self
    }
    /// The same stack with the map points named.
    #[must_use]
    pub fn points(mut self, label: &'static str) -> Self {
        self.points = label;
        self
    }
    /// The same stack with event kind `e` added: an [`EventKind`], or an [`EventShape`] of a crate that declares
    /// its event types as [`EventDef`] ([`EventShape::of`]).
    #[must_use]
    pub fn event(mut self, e: impl Into<EventShape>) -> Self {
        self.events.push(e.into());
        self
    }
    /// The same stack with these settings added after those it has.
    #[must_use]
    pub fn settings(mut self, settings: &[&'static dyn DeclareSetting]) -> Self {
        self.settings.extend_from_slice(settings);
        self
    }
    /// The same stack with command `c` added: a [`CommandKind`], or a [`Command`] without arguments and reply.
    #[must_use]
    pub fn command(mut self, c: impl Into<CommandShape>) -> Self {
        self.commands.push(c.into());
        self
    }
    /// The settings schema of the stack.
    ///
    /// # Panics
    ///
    /// If [`Schema::of`] refuses the settings.
    pub fn schema(&self) -> Schema {
        Schema::of(&self.settings)
    }
    /// The commands of the stack.
    ///
    /// # Panics
    ///
    /// If [`Commands::of`] refuses the commands.
    pub fn commands(&self) -> Commands {
        let list: Vec<&Command> = self.commands.iter().map(|c| c.command).collect();
        Commands::of(&list)
    }
    /// What differs between the stack and a receiver that has the settings `schema`, the results `results` and the
    /// commands `commands`: one sentence that names the keys. `None` if the receiver declares exactly what the stack
    /// holds. A receiver that is not made by the graph crate from this description makes this check when it is made.
    pub fn difference(
        &self,
        schema: &Schema,
        results: &ResultsDecl,
        commands: &Commands,
    ) -> Option<String> {
        if schema.settings() != self.schema().settings() {
            return Some("the settings".into());
        }
        let want = self.results_decl();
        if *results != want {
            let keys = |d: &ResultsDecl| -> Vec<String> {
                let mut k: Vec<String> = d
                    .fields()
                    .fields()
                    .iter()
                    .map(|f| f.key.to_string())
                    .collect();
                k.extend(d.tables().iter().map(|t| format!("table {}", t.key)));
                k.extend(d.series_decls().iter().map(|s| format!("series {}", s.key)));
                k.extend(d.rasters().iter().map(|r| format!("raster {}", r.key)));
                k.extend(d.events().iter().map(|e| format!("event {}", e.kind)));
                k
            };
            let (want_keys, have_keys) = (keys(&want), keys(results));
            let extra: Vec<&String> = have_keys
                .iter()
                .filter(|k| !want_keys.contains(k))
                .collect();
            let missing: Vec<&String> = want_keys
                .iter()
                .filter(|k| !have_keys.contains(k))
                .collect();
            return Some(format!(
                "the results (published and not described: {extra:?}; described and not published: {missing:?})"
            ));
        }
        let want: Vec<_> = self.commands.iter().map(|c| c.command.def()).collect();
        if commands.defs() != want.as_slice() {
            let keys = |d: &[crate::CommandDef]| -> Vec<String> {
                d.iter().map(|c| c.key.to_string()).collect()
            };
            return Some(format!(
                "the commands (the receiver has {:?}, the description has {:?})",
                keys(commands.defs()),
                keys(&want)
            ));
        }
        None
    }
    /// The run-time declaration of the results of the stack.
    ///
    /// # Panics
    ///
    /// If a key is declared twice.
    pub fn results_decl(&self) -> ResultsDecl {
        let mut d = ResultsDecl::new();
        let late = |f: &&Shape| self.late_sets.contains(&f.name);
        for f in self.fields.iter().filter(|f| !late(f)) {
            d = d.with_fields(f.fields);
        }
        for c in &self.components {
            d = d.with_fields(c.fields);
        }
        for f in self.fields.iter().filter(late) {
            d = d.with_fields(f.fields);
        }
        for t in &self.tables {
            d = d.with_table(t.def);
        }
        for s in &self.series {
            d = d.with_series(s);
        }
        for r in &self.rasters {
            d = d.with_raster(r);
        }
        if !self.points.is_empty() {
            d = d.points(self.points);
        }
        for e in &self.events {
            d = d.event(e.def);
        }
        d
    }
}

impl std::fmt::Debug for StackShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackShape").field("id", &self.id).finish()
    }
}

/// The interface of a stack family: its key, its version, its stacks, and what each version after 1.0 added.
///
/// An item has a path: `shape.<Name>` for a shape and `shape.<Name>.<key>` for one of its fields, `<stack>` for a stack,
/// `<stack>.fields.<Name>` for a result set, `<stack>.table.<key>`, `<stack>.series.<key>`, `<stack>.raster.<key>`,
/// `<stack>.event.<kind>`, `<stack>.setting.<key>` and `<stack>.command.<key>`, and `<stack>.content` for the kind of
/// content and the media of a stack. An item is of version 1.0 unless
/// [`Family::added`] lists its path.
#[derive(Clone, Debug)]
pub struct Family {
    /// The key (`"adsb"`). The generated files carry it in their names.
    pub key: &'static str,
    /// The version of the interface.
    pub version: Version,
    /// The stacks, in order.
    pub stacks: Vec<StackShape>,
    /// The paths of the items that each later version added.
    pub additions: Vec<(Version, &'static [&'static str])>,
    /// The family describes a kind of object that every host has (a session, a radio, the engine), not stacks. It
    /// holds one entry in `stacks`, whose id is the key of the family.
    pub object: bool,
    /// The names that options have in the generated forms in place of the name from their key or label: the key of the
    /// field or setting, the key of the option, and the name in lower case with `_` between its words.
    pub option_names: Vec<(&'static str, &'static str, &'static str)>,
    /// The family describes event types, tables and result sets that a protocol or service crate declares for the
    /// stacks of several families ([`Family::shared`]). It holds one entry in `stacks`, whose id is the key of the
    /// family.
    pub shared: bool,
    /// The shared interfaces whose items the stacks of the family list ([`Family::uses`]).
    pub uses: Vec<&'static Family>,
    /// The source files and folders of the crate that several stacks use, each with the ids of these stacks
    /// ([`Family::shared_source`]).
    pub shared_sources: Vec<(&'static str, &'static [&'static str])>,
    /// The kinds of content and the media that the family states for its stacks by id ([`Family::content`],
    /// [`Family::audio`], [`Family::pictures`], [`Family::video`], [`Family::text`]).
    pub stack_marks: Vec<(StackMark, &'static [&'static str])>,
}

/// One statement of a family about some of its stacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackMark {
    /// The kind of content.
    Content(ContentKind),
    /// The kind of sound.
    Audio(AudioKind),
    /// Still pictures.
    Pictures,
    /// Moving pictures.
    Video,
    /// Running text for people.
    Text,
}

impl StackMark {
    /// `s` with this statement.
    fn on(self, s: &mut StackShape) {
        match self {
            StackMark::Content(kind) => s.content = Some(kind),
            StackMark::Audio(kind) => s.stated_media.audio = Some(kind),
            StackMark::Pictures => s.stated_media.pictures = true,
            StackMark::Video => s.stated_media.video = true,
            StackMark::Text => s.stated_media.text = true,
        }
    }
}

impl Family {
    /// A family `key` at version `major.minor` with no stack.
    pub fn new(key: &'static str, major: u16, minor: u16) -> Self {
        Family {
            key,
            version: Version { major, minor },
            stacks: Vec::new(),
            additions: Vec::new(),
            object: false,
            option_names: Vec::new(),
            shared: false,
            uses: Vec::new(),
            shared_sources: Vec::new(),
            stack_marks: Vec::new(),
        }
    }
    /// The same family, in which the source file or folder `name` under `src` (`teleprinter.rs` or `teleprinter`,
    /// without `.rs`) holds code of the stacks `stacks`. The check of the source of the crate then knows whose file it
    /// is: it must write through shapes only when every one of these stacks is in the description. A file or folder
    /// with the id of one stack needs no entry.
    #[must_use]
    pub fn shared_source(mut self, name: &'static str, stacks: &'static [&'static str]) -> Self {
        self.shared_sources.push((name, stacks));
        self
    }
    /// The interface `key` of a protocol or service crate at version `major.minor`: the event types, tables and
    /// result sets that the crate declares and that stacks of several families show. Add them with
    /// [`Family::stack`], as one entry with the id `key`. They are written once, into their own header and registry
    /// record. A family whose stacks list them names the interface with [`Family::uses`].
    pub fn shared(key: &'static str, major: u16, minor: u16) -> Self {
        Family {
            shared: true,
            ..Family::new(key, major, minor)
        }
    }
    /// The same family, whose stacks list items of the shared interface `shared` (an event type with
    /// [`StackShape::event`], a table with [`StackShape::table`], a result set with [`StackShape::fields`]). The
    /// generated forms of the family then take the structs of these items from the forms of `shared`.
    ///
    /// ```
    /// use sdrkit_core::interface::{EventKind, Family, Fields, StackShape};
    /// use std::sync::OnceLock;
    ///
    /// /// A tone heard.
    /// #[derive(Fields)]
    /// pub struct Tone {
    ///     /// The frequency of the tone.
    ///     #[field(label = "Tone", unit = "Hz")]
    ///     pub tone_hz: f64,
    /// }
    /// pub static TONE: EventKind<Tone> = EventKind::new("tones.tone", "Tone");
    ///
    /// /// In the protocol crate.
    /// pub fn interface() -> &'static Family {
    ///     static SHARED: OnceLock<Family> = OnceLock::new();
    ///     SHARED.get_or_init(|| Family::shared("tones", 1, 0).stack(StackShape::new("tones").event(&TONE)))
    /// }
    ///
    /// // In a stack crate.
    /// let family = Family::new("trunk", 1, 0)
    ///     .uses(interface())
    ///     .stack(StackShape::new("ltr").event(&TONE));
    /// assert_eq!(family.owner_of_event("tones.tone").key, "tones");
    /// assert_eq!(family.owner_of_shape("Tone").key, "tones");
    /// ```
    ///
    /// # Panics
    ///
    /// If `shared` is not made with [`Family::shared`].
    #[must_use]
    pub fn uses(mut self, shared: &'static Family) -> Self {
        assert!(shared.shared, "{} is not a shared interface", shared.key);
        if !self.uses.iter().any(|u| u.key == shared.key) {
            self.uses.push(shared);
        }
        self
    }
    /// The interface that describes the event type `kind`: a shared interface that the family uses, else the family.
    pub fn owner_of_event(&self, kind: &str) -> &Family {
        self.uses
            .iter()
            .copied()
            .find(|u| {
                u.stacks
                    .iter()
                    .any(|s| s.events.iter().any(|e| e.def.kind == kind))
            })
            .unwrap_or(self)
    }
    /// The interface that describes the shape with the struct name `name`: a shared interface that the family uses,
    /// else the family.
    pub fn owner_of_shape(&self, name: &str) -> &Family {
        self.uses
            .iter()
            .copied()
            .find(|u| {
                u.stacks.iter().any(|s| {
                    s.fields.iter().any(|f| f.name == name)
                        || s.components
                            .iter()
                            .any(|c| c.shape.is_some_and(|f| f.name == name))
                        || s.tables.iter().any(|t| t.row.name == name)
                        || s.events.iter().any(|e| e.fields.name == name)
                })
            })
            .unwrap_or(self)
    }
    /// The interface of the kind of object `key` that every host has, at version `major.minor`. Add what the object
    /// declares with [`Family::stack`], as one entry with the id `key`.
    pub fn object(key: &'static str, major: u16, minor: u16) -> Self {
        Family {
            object: true,
            ..Family::new(key, major, minor)
        }
    }
    /// The same family, in which the option `option` of the field or setting `field` has the name `name` in the
    /// generated forms. Give a name where the key or the label of an option reads badly as a name (`F80T0.25Q-1B`).
    #[must_use]
    pub fn option_name(
        mut self,
        field: &'static str,
        option: &'static str,
        name: &'static str,
    ) -> Self {
        self.option_names.push((field, option, name));
        self
    }
    /// The same family with stack `s` added.
    #[must_use]
    pub fn stack(mut self, mut s: StackShape) -> Self {
        for (mark, ids) in &self.stack_marks {
            if ids.contains(&s.id) {
                mark.on(&mut s);
            }
        }
        self.stacks.push(s);
        self
    }
    /// The same family with `mark` stated for the stacks `ids`: those that the family holds, and those that a later
    /// [`Family::stack`] adds.
    fn marked(mut self, mark: StackMark, ids: &'static [&'static str]) -> Self {
        for s in self.stacks.iter_mut().filter(|s| ids.contains(&s.id)) {
            mark.on(s);
        }
        self.stack_marks.push((mark, ids));
        self
    }
    /// The same family, in which the stacks `ids` have the kind of content `kind`. It is the form of
    /// [`StackShape::content`] for a family that states the kinds of its stacks in one place. The call can stand before
    /// the stacks are added.
    #[must_use]
    pub fn content(self, kind: ContentKind, ids: &'static [&'static str]) -> Self {
        self.marked(StackMark::Content(kind), ids)
    }
    /// The same family, in which the stacks `ids` give sound of kind `kind` ([`StackShape::audio`]).
    #[must_use]
    pub fn audio(self, kind: AudioKind, ids: &'static [&'static str]) -> Self {
        self.marked(StackMark::Audio(kind), ids)
    }
    /// The same family, in which the stacks `ids` give still pictures ([`StackShape::pictures`]).
    #[must_use]
    pub fn pictures(self, ids: &'static [&'static str]) -> Self {
        self.marked(StackMark::Pictures, ids)
    }
    /// The same family, in which the stacks `ids` give moving pictures ([`StackShape::video`]).
    #[must_use]
    pub fn video(self, ids: &'static [&'static str]) -> Self {
        self.marked(StackMark::Video, ids)
    }
    /// The same family, in which the stacks `ids` give running text for people ([`StackShape::text`]).
    #[must_use]
    pub fn text(self, ids: &'static [&'static str]) -> Self {
        self.marked(StackMark::Text, ids)
    }
    /// The same family with the items at `paths` marked as added in version `major.minor`.
    #[must_use]
    pub fn added(mut self, major: u16, minor: u16, paths: &'static [&'static str]) -> Self {
        self.additions.push((Version { major, minor }, paths));
        self
    }
    /// The same family after `add`, with every item that `add` brings marked as added in version `major.minor`: a
    /// shared interface in use, a stack, a result set, a table, a series, a raster, an event type, a setting, a
    /// command, a component set, a kept item, a shape and a field of a shape. Use it in place of a list of paths for
    /// [`Family::added`] when a version adds whole stacks.
    ///
    /// ```
    /// use sdrkit_core::interface::{Family, Fields, StackShape};
    ///
    /// /// The counts of a stack.
    /// #[derive(Fields)]
    /// struct Counts {
    ///     /// The good frames.
    ///     #[field(label = "Frames good")]
    ///     frames_ok: i64,
    /// }
    ///
    /// let family = Family::new("demo", 1, 1)
    ///     .stack(StackShape::new("first"))
    ///     .added_in(1, 1, |f| f.stack(StackShape::new("second").fields::<Counts>()));
    /// assert_eq!(family.since("first").minor, 0);
    /// assert_eq!(family.since("second").minor, 1);
    /// assert_eq!(family.since("second.fields.Counts").minor, 1);
    /// assert_eq!(family.since("shape.Counts.frames_ok").minor, 1);
    /// ```
    #[must_use]
    pub fn added_in(self, major: u16, minor: u16, add: impl FnOnce(Family) -> Family) -> Self {
        let before = self.paths();
        let mut after = add(self);
        let new: Vec<&'static str> = after
            .paths()
            .into_iter()
            .filter(|p| !before.contains(p))
            .map(keep)
            .collect();
        after
            .additions
            .push((Version { major, minor }, Box::leak(new.into_boxed_slice())));
        after
    }
    /// The path of every item of the family, as [`Family::added`] takes them.
    fn paths(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .uses
            .iter()
            .map(|u| format!("uses.{}", u.key))
            .collect();
        let shape = |out: &mut Vec<String>, s: &Shape| {
            out.push(format!("shape.{}", s.name));
            out.extend(
                s.fields
                    .iter()
                    .map(|f| format!("shape.{}.{}", s.name, f.key())),
            );
        };
        for s in &self.stacks {
            let id = s.id;
            out.push(id.to_owned());
            if s.content.is_some() {
                out.push(format!("{id}.content"));
            }
            for f in &s.fields {
                out.push(format!("{id}.fields.{}", f.name));
                shape(&mut out, f);
            }
            for c in &s.components {
                match &c.shape {
                    Some(f) => {
                        out.push(format!("{id}.component.{}.fields.{}", c.component, f.name));
                        shape(&mut out, f);
                    }
                    None => out.push(format!("{id}.component.{}.dropped", c.component)),
                }
            }
            out.extend(s.kept.iter().map(|k| format!("{id}.kept.{k}")));
            for t in &s.tables {
                out.push(format!("{id}.table.{}", t.def.key));
                shape(&mut out, &t.row);
            }
            out.extend(s.series.iter().map(|x| format!("{id}.series.{}", x.key)));
            out.extend(s.rasters.iter().map(|x| format!("{id}.raster.{}", x.key)));
            for e in &s.events {
                out.push(format!("{id}.event.{}", e.def.kind));
                shape(&mut out, &e.fields);
            }
            out.extend(
                s.settings
                    .iter()
                    .map(|x| format!("{id}.setting.{}", x.key())),
            );
            for c in &s.commands {
                out.push(format!("{id}.command.{}", c.command.key));
                shape(&mut out, &c.reply);
            }
        }
        out.sort();
        out.dedup();
        out
    }
    /// The stack `id`.
    pub fn stack_shape(&self, id: &str) -> Option<&StackShape> {
        self.stacks.iter().find(|s| s.id == id)
    }
    /// The version in which the item at `path` appeared.
    pub fn since(&self, path: &str) -> Version {
        self.additions
            .iter()
            .find(|(_, paths)| paths.contains(&path))
            .map_or(Version { major: 1, minor: 0 }, |(v, _)| *v)
    }
    /// The indices of the fields of `shape` in the order of a fixed-layout struct: by the version in which a field
    /// appeared, then by its place in the shape. A field added later comes after every earlier field, so a struct only
    /// grows at its end.
    pub fn layout_order(&self, shape: &Shape) -> Vec<usize> {
        let mut order: Vec<(Version, usize)> = shape
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| (self.since(&format!("shape.{}.{}", shape.name, f.key())), i))
            .collect();
        order.sort();
        order.into_iter().map(|(_, i)| i).collect()
    }
}

/// The shared interfaces that `families` use, each once, in the order of their first use.
pub fn shared_of(families: &[&'static Family]) -> Vec<&'static Family> {
    let mut out: Vec<&'static Family> = Vec::new();
    for u in families.iter().flat_map(|f| &f.uses) {
        if !out.iter().any(|o| o.key == u.key) {
            out.push(u);
        }
    }
    out
}

/// True if `a` and `b` are the same text. The derive uses it in a constant to compare keys.
pub const fn same_text(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
