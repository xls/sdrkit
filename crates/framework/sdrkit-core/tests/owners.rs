//! Owners, commands, column declarations and change counters.

use sdrkit_core::command::{Command, CommandError, CommandId, CommandSet};
use sdrkit_core::owner::{self, Owner, OwnerKey, OwnerKeyError};
use sdrkit_core::publish::{field, Field, Published, TableDef};
use sdrkit_core::{
    Argument, Codec, ContentType, FieldDecl, FieldId, Record, RecordDecl, Schema, Setting,
    SettingError, SettingId, SettingSet, Value, ValueKind, Values,
};
use std::sync::Arc;

const FROM: Setting<field::Hz> = Setting::new("from", "From", "Hz", 88e6).range(0.0, 6e9, 1.0);
const STEP: Setting<field::Hz> = Setting::new("step", "Step", "Hz", 100e3).range(1.0, 1e6, 1.0);
const START: Command = Command::new("sweep.start", "Start").args(&[&FROM, &STEP]);
const STOP: Command = Command::new("sweep.stop", "Stop");

sdrkit_core::commands! {
    enum SweepCommand {
        Start = START,
        Stop = STOP,
    }
}

const GAIN: Setting<field::F64> = Setting::new("gain_db", "Gain", "dB", 0.0).range(0.0, 50.0, 1.0);

sdrkit_core::settings! {
    enum SweepSetting {
        Gain = GAIN,
    }
}

struct Sweep {
    values: Values,
    invoked: usize,
    applied: usize,
    span: Option<(f64, f64)>,
}

impl Sweep {
    fn new() -> Self {
        Sweep {
            values: Values::defaults(&SweepSetting::schema()),
            invoked: 0,
            applied: 0,
            span: None,
        }
    }
}

impl Owner for Sweep {
    fn schema(&self) -> &Arc<Schema> {
        self.values.schema()
    }
    fn values(&self) -> Option<&Values> {
        Some(&self.values)
    }
    fn apply(&mut self, id: SettingId, value: &Value) -> Result<(), SettingError> {
        self.applied += 1;
        match SweepSetting::from_id(id).ok_or(SettingError::NoSuchId { id })? {
            SweepSetting::Gain => self.values.set(id, value),
        }
    }
    fn commands(&self) -> &sdrkit_core::Commands {
        SweepCommand::commands()
    }
    fn invoke(&mut self, id: CommandId, args: &Values) -> Result<Option<Record>, CommandError> {
        self.invoked += 1;
        match SweepCommand::from_id(id).ok_or(CommandError::NoSuchId { id })? {
            SweepCommand::Start => self.span = Some((args.get(&FROM), args.get(&STEP))),
            SweepCommand::Stop => self.span = None,
        }
        Ok(None)
    }
}

fn args_of(o: &Sweep, id: CommandId) -> Record {
    o.commands().def(id).map(|d| d.new_args()).unwrap()
}

#[test]
fn a_bad_argument_is_refused_before_invoke() {
    let mut sweep = Sweep::new();
    let id = SweepCommand::Start.id();
    let mut given = args_of(&sweep, id);
    given.set_value(FieldId(1), &Value::Hz(5e6)).unwrap();
    let err = owner::invoke(&mut sweep, id, &given).unwrap_err();
    assert!(matches!(
        err,
        CommandError::BadArgument {
            arg: SettingId(1),
            error: SettingError::OutOfRange { .. }
        }
    ));
    assert_eq!(sweep.invoked, 0);
}

#[test]
fn an_absent_argument_takes_its_default() {
    let mut sweep = Sweep::new();
    let id = SweepCommand::Start.id();
    let mut given = args_of(&sweep, id);
    given.set_value(FieldId(0), &Value::Hz(144e6)).unwrap();
    assert_eq!(owner::invoke(&mut sweep, id, &given), Ok(None));
    assert_eq!(sweep.span, Some((144e6, 100e3)));
}

#[test]
fn arguments_of_another_command_are_refused() {
    let mut sweep = Sweep::new();
    let given = args_of(&sweep, SweepCommand::Stop.id());
    let id = SweepCommand::Start.id();
    assert_eq!(
        owner::invoke(&mut sweep, id, &given),
        Err(CommandError::ArgumentsDecl { id })
    );
}

#[test]
fn a_command_key_resolves_and_the_id_round_trips() {
    for c in [SweepCommand::Start, SweepCommand::Stop] {
        assert_eq!(SweepCommand::from_id(c.id()), Some(c));
    }
    assert_eq!(SweepCommand::from_key("sweep.stop"), Ok(SweepCommand::Stop));
    assert_eq!(
        SweepCommand::from_key("sweep.pause"),
        Err(CommandError::Unknown {
            key: "sweep.pause".into()
        })
    );
    assert_eq!(SweepCommand::from_id(CommandId(2)), None);
}

#[test]
fn a_setting_is_checked_before_apply() {
    let mut sweep = Sweep::new();
    let id = SweepSetting::Gain.id();
    assert!(matches!(
        owner::apply(&mut sweep, id, &Value::F64(99.0)),
        Err(SettingError::OutOfRange { .. })
    ));
    assert_eq!(sweep.applied, 0);
    assert_eq!(owner::apply(&mut sweep, id, &Value::F64(20.0)), Ok(()));
    assert_eq!(sweep.values().map(|v| v.get(&GAIN)), Some(20.0));
}

struct Small;
impl Owner for Small {}

#[test]
fn a_small_owner_needs_no_code_for_what_it_lacks() {
    let mut small = Small;
    assert!(small.schema().is_empty());
    assert!(small.values().is_none());
    assert!(small.results().decl().is_empty());
    assert!(small.events().is_empty());
    assert!(small.commands().is_empty());
    let empty = Record::new(Arc::new(RecordDecl::default()));
    assert_eq!(
        owner::invoke(&mut small, CommandId(0), &empty),
        Err(CommandError::NoSuchId { id: CommandId(0) })
    );
    let dynamic: &dyn Owner = &small;
    assert!(dynamic.commands().is_empty());
}

const NAME: Field<field::Text> = Field::new("name", "Name", "");
const LEVEL: Field<field::F64> = Field::new("level_db", "Level", "dB");
static HITS: TableDef = TableDef {
    key: "hits",
    label: "Hits",
    columns: &[&NAME, &LEVEL],
    cap: 8,
    marks: sdrkit_core::RowMarks::NONE,
};
static OTHER: TableDef = TableDef {
    key: "other",
    label: "Other",
    columns: &[&NAME],
    cap: 8,
    marks: sdrkit_core::RowMarks::NONE,
};

#[test]
fn one_changed_row_returns_one_row_and_an_equal_write_none() {
    let mut p = Published::new(&[&LEVEL]).with_tables(&[&HITS, &OTHER]);
    for (key, level) in [("a", 1.0), ("b", 2.0), ("c", 3.0)] {
        let mut row = p.row_mut(&HITS, key).unwrap();
        row.set(&NAME, key);
        row.set(&LEVEL, level);
    }
    p.row_mut(&OTHER, "x").unwrap().set(&NAME, "x");
    let seen = p.results().change_counter();

    let mut row = p.row_mut(&HITS, "b").unwrap();
    row.set(&LEVEL, 2.0);
    row.set(&NAME, "b");
    p.set(&LEVEL, 0.0);
    p.set(&LEVEL, 0.0);
    let p0 = p.results().change_counter();
    p.set(&LEVEL, 0.0);
    assert_eq!(p.results().change_counter(), p0);
    let hits = p.results().decl().table_id("hits").unwrap();
    let table = p.results().table(hits).unwrap();
    assert_eq!(table.rows_changed_since(seen).count(), 0);
    assert_eq!(p.results().tables_changed_since(seen).count(), 0);

    p.row_mut(&HITS, "b").unwrap().set(&LEVEL, 2.5);
    let table = p.results().table(hits).unwrap();
    let changed: Vec<&str> = table.rows_changed_since(seen).map(|(k, _)| k).collect();
    assert_eq!(changed, ["b"]);
    let tables: Vec<_> = p.results().tables_changed_since(seen).collect();
    assert_eq!(tables, [hits]);
    assert!(p.results().change_counter() > seen);

    let snapshot = p.results().clone();
    assert_eq!(snapshot.change_counter(), p.results().change_counter());
}

#[test]
fn a_removed_row_moves_the_table_counter() {
    let mut p = Published::new(&[]).with_tables(&[&HITS]);
    p.row_mut(&HITS, "a").unwrap().set(&NAME, "a");
    let seen = p.results().change_counter();
    let hits = p.results().decl().table_id("hits").unwrap();
    p.results_mut().table_mut(hits).unwrap().remove("a");
    assert_eq!(p.results().tables_changed_since(seen).count(), 1);
}

const PARENT: Field<field::Text> =
    Field::new("channel", "Channel", "").refers_to("service.bank", "channels");
const SIBLING: Field<field::Text> = Field::new("hit", "Hit", "").refers_to_table("hits");
const LOGO: Field<field::Bytes> =
    Field::new("logo", "Logo", "").content(ContentType::Media(Codec::Png));

#[test]
fn a_reference_and_a_content_type_survive_a_serialise() {
    let decl = RecordDecl::of(&[&PARENT, &LOGO, &SIBLING]);
    let json = serde_json::to_string(&decl).unwrap();
    let back: RecordDecl = serde_json::from_str(&json).unwrap();
    assert_eq!(back, decl);
    let r = back.fields()[0].refers_to.as_ref().unwrap();
    assert_eq!(
        (r.owner.as_ref().map(OwnerKey::as_str), r.table.as_ref()),
        (Some("service.bank"), "channels")
    );
    let same = back.fields()[2].refers_to.as_ref().unwrap();
    assert_eq!((same.owner.as_ref(), same.table.as_ref()), (None, "hits"));
    assert!(!json.contains("\"owner\":null"));
    assert_eq!(
        back.fields()[1].content,
        Some(ContentType::Media(Codec::Png))
    );
    assert_eq!(back.fields()[0].content, None);
}

#[test]
fn an_owner_key_follows_its_rules() {
    assert!(OwnerKey::new("stack.dab").is_ok());
    assert!(OwnerKey::new("stack.tv-auto").is_ok());
    assert_eq!(
        OwnerKey::new("stack.-tv"),
        Err(OwnerKeyError::BadByte { at: 6 })
    );
    assert_eq!(
        OwnerKey::new("stack.tv auto"),
        Err(OwnerKeyError::BadByte { at: 8 })
    );
    assert_eq!(OwnerKey::new(""), Err(OwnerKeyError::Empty));
    assert_eq!(
        OwnerKey::new("stack..dab"),
        Err(OwnerKeyError::EmptyPart { at: 6 })
    );
    assert_eq!(
        OwnerKey::new("Stack"),
        Err(OwnerKeyError::BadByte { at: 0 })
    );
    assert!(serde_json::from_str::<OwnerKey>("\"svc.\"").is_err());
}

const DATA: Field<field::Bytes> = Field::new("data", "Data", "");
const LEVEL_ARG: Field<field::F64> = Field::new("level", "Level", "dB");
const PUT: Command = Command::new("store.put", "Put").args(&[
    &Argument::new(DATA),
    &Argument::new(LEVEL_ARG),
    &STEP,
]);

#[test]
fn a_required_argument_must_be_given_and_bytes_pass() {
    let commands = sdrkit_core::Commands::of(&[&PUT]);
    let id = CommandId(0);
    let def = commands.def(id).unwrap();
    let mut given = def.new_args();
    given.set(&LEVEL_ARG, 3.0);
    assert_eq!(
        commands.check(id, &given),
        Err(CommandError::MissingArgument { arg: SettingId(0) })
    );
    let bytes: Arc<[u8]> = Arc::from(&b"abc"[..]);
    given.set(&DATA, Arc::clone(&bytes));
    let args = commands.check(id, &given).unwrap();
    assert_eq!(args.arg(&Argument::new(DATA)), Ok(&b"abc"[..]));
    assert_eq!(args.arg(&Argument::new(LEVEL_ARG)), Ok(3.0));
    assert_eq!(args.get(&STEP), 100e3);
}

#[test]
fn an_undeclared_argument_is_not_an_argument() {
    const OTHER: Field<field::F64> = Field::new("other", "Other", "");
    let commands = sdrkit_core::Commands::of(&[&PUT]);
    let id = CommandId(0);
    let mut given = commands.def(id).unwrap().new_args();
    given.set(&DATA, Arc::from(&b"x"[..]));
    given.set(&LEVEL_ARG, 1.0);
    let args = commands.check(id, &given).unwrap();
    assert_eq!(
        args.arg(&Argument::new(OTHER)),
        Err(CommandError::NotAnArgument { key: "other" })
    );
}

#[test]
fn a_setting_cannot_be_required_or_bytes() {
    let required = sdrkit_core::SettingDecl::required(FieldDecl::new("x", "X", "", ValueKind::F64));
    assert!(Schema::new(vec![required.clone()]).is_err());
    assert!(Schema::arguments(vec![required]).is_ok());
    let bytes = sdrkit_core::SettingDecl::new(
        FieldDecl::new("b", "B", "", ValueKind::Bytes),
        Value::Bytes(Arc::from(&[][..])),
    );
    assert!(Schema::new(vec![bytes]).is_err());
}

#[test]
fn a_content_type_on_a_text_column_is_refused() {
    let text = FieldDecl::new("name", "Name", "", ValueKind::Text).with_content(ContentType::Csv);
    assert_eq!(
        RecordDecl::new(vec![text]),
        Err(sdrkit_core::record::DeclError::ContentNotBytes { key: "name".into() })
    );
    let file = FieldDecl::new("file", "File", "", ValueKind::Bytes).with_content(ContentType::Json);
    assert!(RecordDecl::new(vec![file]).is_ok());
}

#[test]
fn two_results_have_separate_counters() {
    let mut a = Published::new(&[&LEVEL]);
    let mut b = Published::new(&[&LEVEL]);
    a.set(&LEVEL, 1.0);
    a.set(&LEVEL, 2.0);
    assert_eq!(a.results().change_counter(), 2);
    assert_eq!(b.results().change_counter(), 0);
    b.set(&LEVEL, 2.0);
    assert_eq!(b.results().change_counter(), 1);
    assert_eq!(a.results().change_counter(), 2);
}

#[test]
fn a_column_of_any_kind_holds_values_of_two_kinds() {
    const KEY: Field<field::Text> = Field::new("key", "Key", "");
    const VALUE: Field<field::Any> = Field::new("value", "Value", "");
    const STORED: TableDef = TableDef {
        key: "stored",
        label: "Stored",
        columns: &[&KEY, &VALUE],
        cap: 10,
        marks: sdrkit_core::RowMarks::NONE,
    };
    let mut p = Published::new(&[]).with_tables(&[&STORED]);
    p.row(
        &STORED,
        "a",
        vec![Value::Text("width".into()), Value::F64(8000.0)],
    );
    p.row_mut(&STORED, "b")
        .unwrap()
        .set(&VALUE, &Value::Choice(3));
    let t = p
        .results()
        .table(p.results().decl().table_id("stored").unwrap())
        .unwrap();
    assert_eq!(t.row("a").unwrap().get(&VALUE), Some(&Value::F64(8000.0)));
    assert_eq!(t.row("b").unwrap().get(&VALUE), Some(&Value::Choice(3)));
    assert_eq!(t.columns().field(FieldId(1)).unwrap().kind, ValueKind::Any);

    // A typed column still refuses another kind.
    let mut r = Record::new(Arc::clone(t.columns()));
    assert!(r.set_value(FieldId(0), &Value::F64(1.0)).is_err());
    assert_eq!(r.set_value(FieldId(1), &Value::Bool(true)), Ok(true));

    // A setting cannot have the kind, a command argument can.
    let any = FieldDecl::new("v", "V", "", ValueKind::Any);
    assert!(Schema::new(vec![sdrkit_core::SettingDecl::new(
        any.clone(),
        Value::I64(0)
    )])
    .is_err());
    let args = Arc::new(Schema::of_arguments(&[&Argument::<field::Any>::new(VALUE)]));
    let mut v = Values::defaults(&args);
    assert_eq!(v.set(SettingId(0), &Value::Hz(1e6)), Ok(()));
    assert_eq!(
        v.set(SettingId(0), &Value::None),
        Err(SettingError::Required { id: SettingId(0) })
    );
}
