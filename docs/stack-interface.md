# How to describe a stack's interface

A stack family is one stack crate (`sdrkit-stack-<family>`). Its interface is everything that a consumer reads or
calls: result fields, tables, series, map points, events, settings and commands. You write the interface once, as Rust
structs and one description. The run-time declarations, the C header, the Swift file, the registry record and the
tables of the docs pages are written from it.

## What you write

Put the interface in `src/typed.rs` of the stack crate.

### Shapes

A shape is a struct with `#[derive(Fields)]`. It describes a table row, a result set, the fields of an event or the
reply of a command. The name of a struct field is the key. Its type gives the kind. `Option` marks a field that can be
without a value. The doc comment is the description that people read.

```rust
use sdrkit_core::interface::{Fields, Hertz, Time};

/// A row of the ship table. The row key is the MMSI.
#[derive(Clone, Debug, PartialEq, Fields)]
pub struct ShipRow<'a> {
    /// The name of the ship.
    #[field(label = "Name")]
    pub name: Option<&'a str>,
    /// The speed over the ground.
    #[field(label = "Speed", unit = "kt")]
    pub speed_kt: Option<f64>,
    /// The channel frequency.
    #[field(label = "Frequency", unit = "Hz")]
    pub freq_hz: Hertz,
    /// A standard key: the declaration comes from the constant.
    #[field(shared = sdrkit_core::publish::keys::POSITION)]
    pub position: Option<(f64, f64)>,
}
```

| Rust type | Kind | C member |
|---|---|---|
| `&str` | text | `sdrkit_string` |
| `i64` | whole number | `int64_t` |
| `f64` | number | `double` |
| `bool` | yes or no | `bool` |
| `Hertz`, `Time`, `Seconds` | frequency, time, length of time | `double` |
| `(f64, f64)` | position, latitude first | `sdrkit_position` |
| an enum of `choice_enum!` | one of a list of options | `int32_t`, with a C enum of the options |
| `Arc<[u8]>` | bytes | `sdrkit_bytes` |
| `&Value` | a value that carries its own kind, for a column whose rows hold values of several kinds | `sdrkit_variant` |

Attributes of a field: `label` (required unless `shared`), `unit`, `refers_to = "table"` for a row key of another
table, `content = PATH` for what a bytes field holds (a constant of the content type), and `shared = PATH` for a field
that a constant of a shared crate declares.

### Tables, event types and commands

```rust
use sdrkit_core::interface::{Arguments, CommandKind, EventKind, Table};

/// The ships.
pub static SHIPS: Table<ShipRow<'static>> = Table::new("ships", "Ships", 300);

/// A ship was heard for the first time.
pub static SHIP_SEEN: EventKind<ShipRow<'static>> = EventKind::new("ais.ship", "Ship seen");

/// The arguments of `remove`.
#[derive(Arguments)]
pub struct RemoveArgs<'a> {
    /// The row key of the ship.
    #[arg(label = "Row", refers_to = "ships")]
    pub row: &'a str,
}

/// Forgets one ship.
pub const REMOVE: CommandKind<RemoveArgs<'static>> = CommandKind::new("remove", "Remove");
```

One event key can have a shape for each stack. The stacks of a family that show the same fields share one
`EventKind`. A stack whose events of that key have other fields declares its own `EventKind` with the same key and
its own shape, and lists it:

```rust
/// A decode of a slot stack.
pub static DECODE: EventKind<DecodeEvent<'static>> = EventKind::new("wsjt.decode", "Decode");
/// A ping of `jt6m`.
pub static JT6M_DECODE: EventKind<Jt6mDecodeEvent<'static>> = EventKind::new("wsjt.decode", "Ping");
```

The shape that the first stack of the family lists the key with keeps the name of the key. Each other shape belongs
to one stack and has the id of that stack in its names. The writer refuses a second shape that several stacks list.

A struct field of a plain type is a required argument. A struct field of type `Option` is an optional argument. It
needs a default, and it can have limits:

```rust
/// The addressee.
#[arg(label = "To", default = "^all")]
pub to: Option<&'a str>,
/// The packet that the text replies to.
#[arg(label = "Reply to", default = 0, min = 0, max = 4_294_967_295.0, step = 1)]
pub reply_id: Option<i64>,
```

A caller that passes `None` leaves the argument out, and the default is in force. The owner that runs the command
always reads `Some`. `#[arg(shared = PATH)]` names an `Argument` constant for a required argument and a `Setting`
constant for an optional one. In C, an optional argument has a `bool has_<key>` member before it. In Swift, it is an
optional parameter that is `nil` by default.

A command with a reply names the shape of the reply as the second type:
`CommandKind<RemoveArgs<'static>, Removed>`. A command without arguments and without a reply stays a plain `Command`.
List the command in the `commands!` enum of the component that runs it (`Remove = REMOVE.command`), read the
arguments with `REMOVE.args(values)?` and make the reply with `REMOVE.reply(commands, &reply)`.

Settings stay as they are: a `settings!` enum over `Setting` constants.

### The description

```rust
use sdrkit_core::interface::{Family, StackShape};
use sdrkit_core::SettingSet;
use std::sync::OnceLock;

/// The interface of the family.
pub fn family() -> &'static Family {
    static FAMILY: OnceLock<Family> = OnceLock::new();
    FAMILY.get_or_init(|| {
        Family::new("ais", 1, 0).stack(
            StackShape::new("ais")
                .fields::<AisFields>()
                .table(&SHIPS)
                .event(&SHIP_SEEN)
                .settings(AisSetting::SETTINGS)
                .command(&sdrkit_util_objects::commands::CLEAR)
                .command(&REMOVE),
        )
    })
}
```

The key of the family is the name of the crate without `sdrkit-stack-`. The order of the calls is the order that a
generic consumer sees. `.series(..)`, `.raster(..)` and `.points("label")` add the series, the rasters and the map
points of a stack. A result set that several components write is one struct in the description, and each
component writes a part of it: a second struct with `#[fields(part_of = AisFields<'static>)]` whose fields have the
names and types of the same fields of the whole set, without attributes.

### Results of a named component

A component that is not the status component of the graph shows its results under its name: the key of a field or a
table is `<component>.<key>`. List these results with `.component(name, ..)`. The component writes them with its own
shapes and its own keys.

```rust
StackShape::new("inmarsat")
    .fields::<InmarsatFields>()
    .decoder("receiver")
    .component("egc", |c| c.fields::<EgcFields>().dropped().table(&EGC_MESSAGES))
```

- `c.fields::<EgcFields>()`: the fields `egc.<key>`. In C: `sdrkit_inmarsat_egc_fields_read()` fills the struct of
  `EgcFields`.
- `c.table(&EGC_MESSAGES)`: the table `egc.messages`. In C: `sdrkit_inmarsat_egc_messages_rows()`. In Rust:
  `EGC_MESSAGES.rows_of::<EgcMessageRow>("egc", results)`.
- `c.dropped()`, or `.decoder(name)` for a decoder with no other result: the field `<name>.dropped`, which a decoder
  component writes itself. Do not declare it by hand. The counts of all such decoders of a stack are one struct:
  `sdrkit_inmarsat_dropped` with one member for each decoder, read with `sdrkit_inmarsat_dropped_read()`. The status
  decoder has `.fields::<DecoderFields>()` as before.

The fields of the components follow the result sets of the stack in the generic form, in the order of the calls. The
tables are in the order of all `.table(..)` calls. Put the calls in the order of `tests/declarations.txt`.

A result set of the stack whose fields follow those of the components is listed with
`.fields_after_components::<F>()`. A receive chain that wraps a graph and writes fields of its own after the results of the
graph needs it: `dvb_terrestrial` of `sdrkit-stack-dvbt` shows its field `standard` so. In the typed forms the set
is a result set of the stack as any other.

### Fields that a component shares with the stack

A component can write a field that the stack shows under its own key, without the name of the component. Name these
fields after the result set of the component:

```rust
StackShape::new("inmarsat")
    .fields::<InmarsatFields>()                 // declares `frames_ok`
    .component("egc", |c| c.fields::<EgcFields>().shares(&["frames_ok"]))
```

- The field `frames_ok` of `EgcFields` is not `egc.frames_ok`. The component writes the field `frames_ok` of the stack,
  which a result set of the stack declares.
- Several components can share one key. The stack shows the value of the status component if it holds one, else the
  value of the first writer in run order.
- In C, the member of the struct of the component set holds the value of the stack's field.
- `build()` fails if the component does not publish a shared field.
- A shared field can have the key of a table of its component: `messages` is then the field of the stack, and
  `egc.messages` the table of the component.

A table of the stack that components write under its own key is listed once with `.table(&T)`. Name each writer:
`.component("rx", |c| c.writes_table(&T))`. The row type is that of `T`. The rule for the rows is the rule for a
shared field.

### Fields that the receive chain writes itself

A receive chain that is a `StackGraph` counts the input samples that its guard replaced. No component declares this field.
A stack that shows it lists the set at its place among the result sets:

```rust
StackShape::new("gnss").fields::<GnssFields>().receiver_fields::<sdrkit_graph::GuardFields>()
```

The field then holds a value from the start, 0 too. A stack that does not list the set shows `bad_samples` only after
the first replaced sample, as before.

### A change of frequency

When the host changes the frequency of a session, the engine tells each component of the receive chain to forget what it
decoded. This is no command: a component needs no `clear` command for it, and the stack declares no new key. A change
of frequency that a service or the stack itself orders does not clear.

Name what belongs to no single frequency, and must stay:

```rust
StackShape::new("sbas")
    .fields::<SbasFields>()
    .table(&MESSAGES)
    .table(&SATELLITES)
    .kept(&["messages", "SbasFields.sbas_messages"])
```

An entry is one of:

- the key of a table (`messages`, or `egc.messages` for a table of a named component),
- the name of the struct of a result set: every field of the set,
- `<Struct>.<key>`: one field of a result set.

Everything else is cleared, also in a component that holds a kept item. `build()` fails for an entry that names
nothing.

What a component does:

| Component | Nothing of it is kept | Something of it is kept |
|---|---|---|
| A `Decoder` without `forget` | `Decoder::clear` runs | Nothing is cleared |
| A `Decoder` with `forget(&mut self, kept: &Kept)` | `forget` runs | `forget` runs |
| A `Component` without `forget` | Its `clear` command runs, if it has one without a required argument | Nothing is cleared |
| A `Component` with `forget` | `forget` runs | `forget` runs |

So a component that holds a kept item and other results implements `forget`. It clears the other results and tests
each kept one:

```rust
fn forget(&mut self, kept: &Kept) {
    self.satellites.clear();
    if !kept.table(MESSAGES.key) {
        self.log.clear();
    }
    if !kept.field("sbas_messages") {
        self.total = 0;
    }
}
```

`kept` holds the keys of the component's own fields and tables. The `sbas` stack of `sdrkit-stack-gnss` is the
example: the message log and the count of messages stay, the satellites and their corrections go.

A stack whose results belong to one frequency also clears on a retune that the host did not order (a scan step, a
retune that a service asks for). List the components with `.on_retune(CommandTarget::of("report", "clear"))` on the
builder. On each retune:

- the command `clear` does not run in a component of which the stack keeps a field or a table. The component runs
  `forget` with its kept keys, as above: the kept items stay. Such a component implements `forget`;
- every other command of the list runs with its default arguments.

So a kept table survives every retune, ordered by the host or not, and a component needs no second rule for it.

### Publishing

A component fills a struct. It cannot write one cell, and it cannot put a row into a table of another shape.

| What | Call |
|---|---|
| Result fields | `published.put(&AisFields { .. })` |
| One table row | `SHIPS.put(&mut published, key, &ShipRow { .. })` |
| The rows of an `ObjectTable` | `impl Shaped for Ship { type Row = ShipRow<'static>; fn row(..) }`, then `shaped::rows(&ships, &SHIPS, now, &mut published)` |
| Rows keyed by their place (`000`, `001`, ...) | `shaped::indexed_rows(&mut published, &CARRIERS, &carriers, \|c, out\| out.put(&CarrierRow { .. }))` |
| An event | `SHIP_SEEN.event(time, freq_hz, &ShipRow { .. })` |

`Shaped` and the functions that take it are in `sdrkit_util_objects::shaped`. To read one field, use the constant that
the derive makes: `record.at(&ShipRow::NAME)`. To read a whole row or set, use `SHIPS.rows::<ShipRow>(results)` or
`AisFields::read(results.fields())`.

### A result set states the present

A component fills its whole result set on each report. The set says what is true now. It is not a list of changes.

- A measurement that does not exist now has no value. A stack that measures the period of a sync word only while it
  has sync writes `sync_period: None` when the sync is lost, and a consumer then shows no period. A frequency of a
  channel that the stack does not hear now is the same case.
- A value that is still true stays. A decoded text is true until another text replaces it, so the component keeps
  the text in its own state and writes it on each report: `text: self.text.as_deref()`.

The framework does not keep a last value for a field that a report leaves out. If a consumer must see the last value
of a measurement, the stack declares a second field for it (`last_sync_period`) and fills it from its own state.

### The receive chain

Build the receive chain from the description. The id, the settings schema and the declaration of the results come from it.

```rust
StackGraph::of(shape, graph)
    .route("expiry_s", &["tracker.expiry_s"])?
    .command("clear", CommandTarget::of("tracker", "clear"))
    .command("remove", CommandTarget::of("tracker", "remove"))
    .with_settings(&cfg.settings)
    .build()
```

`build()` returns `GraphError::Shape` if the settings, the results or the commands of the receive chain differ from the
description, or if no component declares a field, a table, a series or a raster of the description. The error names
the keys that differ. A receive chain that is not a `StackGraph` makes the same comparison with
`StackShape::difference` when it is made.

### The catalog

The catalog needs no line. Its build script finds `src/typed.rs` in a stack crate, and the family list then holds
`sdrkit_stack_<family>::typed::family()`. The key of the family is the name of the crate without `sdrkit-stack-`. The
C library, the registry check and the docs pages take the families from this list.

## What is written from the description

One command writes every file of every family:

```sh
SDRKIT_WRITE_INTERFACE=1 cargo test --no-fail-fast -p sdrkit-host-catalog -p sdrkit-host-ffi -p sdrkit-dev-test \
    --test families --test typed_files --test docs written
```

With `--no-fail-fast`, a family whose record is refused does not stop the files of the other families.

To write only some files, set `SDRKIT_WRITE_ONLY` to parts of their paths with a comma between them
(`SDRKIT_WRITE_ONLY=wsjt,/ft8.md`). The other files are compared and not written.

| File | Test |
|---|---|
| `registry/<family>.reg` | `families` of `sdrkit-host-catalog` |
| `crates/hosts/sdrkit-host-ffi/include/sdrkit/<family>.h` and `swift/<Family>.swift` | `typed_files` of `sdrkit-host-ffi` |
| `crates/hosts/sdrkit-host-ffi/include/sdrkit/init.h`: the `*_INIT` initialisers of the structs of `sdrkit.h` that begin with `size` | `typed_files` of `sdrkit-host-ffi` |
| The part of `docs/stacks/<stack>.md` between the two marker lines | `docs` of `sdrkit-dev-test` |

To check, run the three tests without the variable and without the word `written`. They fail if a file differs from
the description, and the test `families` reads the source of the crate:

```sh
cargo test -p sdrkit-host-catalog -p sdrkit-host-ffi -p sdrkit-dev-test --test families --test typed_files --test docs
```

### Names

| Item | C name |
|---|---|
| The struct of a row, a result set, a reply or arguments | `sdrkit_<scope>_<rest>`. If the name of the Rust struct begins with the id of a stack of the family, the scope is that stack: `AisStationRow` gives `sdrkit_ais_station_row`. If one stack only uses the struct, the scope is that stack: `AddressedRow` of the stack `uplink1030` gives `sdrkit_uplink1030_addressed_row`. If not, the scope is the family, and a name that begins with the family key does not repeat it: `FmFields` gives `sdrkit_fm_fields`. A function of a stack and a struct that only this stack uses then start with the same words. |
| The struct of an event type | `sdrkit_<type>_event`, one struct for each event type: `sdrkit_fm_station_event`. A type whose key ends in `event` does not get the word again: `science.event` has `sdrkit_science_event`. A key that does not begin with the id of a stack of the family or with the key of the family gets one: the id of the stack if one stack only lists the type (`fsk.lock` of `navtex` has `sdrkit_navtex_fsk_lock_event`), the key of the family if several do (`text` of `hfdata` has `sdrkit_hfdata_text_event`). A stack that lists the type with a shape of its own has its own struct, with the id of the stack in place of the scope of the key: `wsjt.decode` of `fsk441` has `sdrkit_fsk441_decode_event`. The key of the generic interface stays as it is. |
| The enum of a member with options | The name of the struct, then the key of the member. A member whose key is the last word of the struct (the member `event` of an event struct) has `<struct>_value`, and its options start with the name of the struct. |
| A table | `sdrkit_<stack>_<table>_rows`. A table with the key of its stack has `sdrkit_<stack>_rows`. A key that is the last words of the stack id is not said twice: the table `events` of `beacon_events` has `sdrkit_beacon_events_rows`. A key that ends in `rows` does not get the word again: `lv_rows` of `astro` has `sdrkit_astro_lv_rows`. |
| A result set of a named component | `sdrkit_<stack>_<component>_<struct>_read`, the struct name without the words of the component at its start: `sdrkit_inmarsat_egc_fields_read`. |
| The dropped counts of the decoders | The struct `sdrkit_<stack>_dropped` and `sdrkit_<stack>_dropped_read`. |
| A result set | The name of its struct, then `_read`. |
| A command | `sdrkit_<stack>_<command>`. |
| A struct member | The key. A key that is a reserved word of C or C++ has `_` after it: the column `default` is the member `default_`, and `class` is `class_`. A test compiles every header as C++17. A key with the name of a member of the head of its struct has `_` after it too: the column `key` of a row is `key_`, after the row key, and `keyField` in Swift. In Swift, the enum of a member `type` or `protocol` is `TypeOption` or `ProtocolOption`. |
| A setting | `sdrkit_<stack>_get_<key>` and `sdrkit_<stack>_set_<key>`. A setting of the test signal generator (`gen_<name>`) has `sdrkit_<stack>_gen_get_<name>` and `sdrkit_<stack>_gen_set_<name>`. |
| An option | The name of the enum, then the word of the option. The word is the key of the option, or the words of the label before the first bracket if the key is a bare number: `SDRKIT_FM_RDS_CORRECTION_OFF`, and `115200` for the label `115200 (Wenet)`. Every character that is not a letter or a digit becomes `_`, and a run of `_` becomes one: the key `F80T0.25Q-1B` gives `F80_T0_25_Q_1_B`. Where that reads badly, the description gives the name of one option: `Family::new(..).option_name("bearer", "F80T0.25Q-1B", "f80_quarter_rate")`, or `.option_name("aspect", "16:9", "wide")` for an option that would be `16_9`. The arguments are the key of the field or setting, the key of the option, and the name. The writer refuses an empty name and a name that two options of one list have. |

The session, the radio and the engine have a description too (`typed.rs` of `sdrkit-host-engine`), made from the
declarations of their owners. A result field of the session has the key `session.<name>`, and its struct member is
`<name>`: a member does not repeat the key of its family. The writer refuses a struct with two members of one name.

The writer refuses a name in which one word stands twice in a row. The verb of a setting function is the exception:
the setting `gen_set` has `sdrkit_<stack>_gen_set_set`. A name is frozen when a library with it is given
to a host: choose the names of the Rust structs with these rules in mind.

The header has a struct for each row, result set, event type, arguments struct and reply; an enum for each member
and setting with options; and a function for each table, result set, event type (`sdrkit_<type>_event_read`),
command and setting. Do not edit a written file.

A docs page keeps the text that you write above the first marker line and below the second.

### Shapes of a shared crate

A protocol or service crate that declares event types, rows or result sets for stacks of several families describes
them once, in that crate:

1. Write each as a shape (`#[derive(Fields)]`) in the crate, and write through the struct.
2. Make each event type from its shape: an `EventKind<S>`, or, where components return events through a declaration,
   `EventDef::new(kind, label, S::SHAPE.fields)` with a function that returns `EventShape::of::<S>(&DEF)`.
3. Add `pub fn interface() -> &'static Family` with `Family::shared("<key>", 1, 0).stack(StackShape::new("<key>")...)`
   that lists the event types, the tables and the result sets. The key is the name of the crate without its role
   prefix.
4. In each family that uses them: `Family::new(..).uses(sdrkit_proto_tones::components::interface())`, and list the
   item in the stack: `.event(sdrkit_proto_tones::components::dtmf())`.

The shared interface has its own files and its own version: `registry/<key>.reg`, `include/sdrkit/<key>.h` and
`swift/<Key>.swift`. The header of a family that uses it includes it and writes none of its structs again. The command
of "What is written from the description" writes these files with those of the first family that uses the interface.

Done: `sdrkit-proto-tones` (`tones`: five event types), `sdrkit-proto-mpegts` (`mpegts`: the transport stream layer,
the monitor and teletext), `sdrkit-proto-ipcast` (`ipcast`: the result sets of its four components), `sdrkit-graph`
(`graph`: the result set of each standard component that publishes results) and `sdrkit-codec-image` (`image`:
`PictureFields`, the result set of the picture component).

A stack that shows the results of a standard component names the interface and lists the set under the name of the
component in its graph:

```rust
Family::new("tvterr", 1, 0).uses(sdrkit_graph::components::interface())
// ...
StackShape::new("isdbt")
    .fields::<IsdbtFields>()
    .component("dc", |c| c.fields::<sdrkit_graph::components::DcRemovalFields>())
```

The sets are `DcRemovalFields`, `IqCorrectionFields`, `NoiseBlankerFields`, `PowerMeterFields`, `SquelchFields`,
`AmDemodFields`, `StackGeneratorFields` and `NetSinkFields`. A field of them has no value before its first
measurement.

A shared crate whose components run in the graphs of many stacks gives one function that adds its items to the
description of a stack. The stack lists its own result sets and tables first:

```rust
let s = StackShape::new("dvbc").fields::<DvbcFields>();
let s = sdrkit_proto_mpegts::monitor::describe(s, "monitor");                    // the counts of the monitor
let s = sdrkit_proto_ipcast::components::describe::<BbFrameFields>(s, "bbframe"); // one component of ipcast
let s = sdrkit_proto_mpegts::layer::describe(s, &TsLayer::all());                // the layer of `layer::attach`
```

These functions use `ComponentShape::shares_declared()`: a field of the component whose key a result set of the stack
declares is the field of the stack, and every other field has the key `<component>.<key>`.

The crates that still declare by hand, with the number of declared fields, event types and tables:

| Crate | Fields | Event types | Tables | Used by |
|---|---|---|---|---|
| `sdrkit-proto-bcdata` | 26 | 0 | 0 | 3 stack crates; it has shapes already, and no shared interface |
| `sdrkit-proto-aero` | 21 | 0 | 1 | 3 stack crates |
| `sdrkit-proto-lora-phy` | 12 | 1 | 0 | 2 stack crates |
| `sdrkit-proto-ookdev` | 9 | 1 | 0 | 1 stack crate |
| `sdrkit-util-objects`, `sdrkit-util-bits` | 5 each | 0 | 1 | standard columns: named with `#[field(shared = ..)]`, no interface needed |
| `sdrkit-svc-sweep`, `-record`, `-scan`, `-memory`, `-identify`, `-freqdb`, `-bank`, `-automation`, `-store`, `-outputs`, `-watch` | 421 | 26 | 65 | each service is an object of the engine: it needs a description of its own, as the session has |
| `sdrkit-driver-core` and the ten driver crates | 231 | 4 | 13 | the radio; `sdrkit-driver-core` is described in `typed.rs` of `sdrkit-host-engine`, a driver's own fields are not |

### Shared publishers

`sdrkit-proto-bcdata` writes the radio text, the guide, the slides and the logos through its own shapes
(`publish::LabelTagRow`, `LABEL_TAG_TABLE` and the others): name them in the description. `sdrkit-proto-aero` returns
the standard columns of an aircraft as one value (`Aircraft::standard`): copy its members into the struct of the row,
whose fields name the constants of `aircraft::columns` with `#[field(shared = ...)]`. The map point of an aircraft is
`Aircraft::standard_point`: give it to `shaped::points_by` with the rows of the aircraft table. A shared
interface of the crate, with its own header and record, is open: the crate declares columns, and each stack has its
own row struct with them. A stack with a decoder component
names `sdrkit_graph::DecoderFields` as its last result set.

## The registry check

The registry record lists every stack, shape, field, table, series, event type, setting and command of the family
with its type, its unit and the version in which it appeared. The test `families` of `sdrkit-host-catalog` compares
the record in the repository with the description:

- No difference: the test passes.
- A compatible difference with a higher minor version, or a breaking difference with a higher major version: the test
  fails until you write the record again with `SDRKIT_WRITE_INTERFACE=1`.
- A difference that the version does not permit: the test fails and the record is not written. The message lists each
  difference with its class and the version to raise.

The same test file reads the source of the crate. It fails if a stack of the description declares a field, a table or
an event type by hand (`Field::new`, `TableDef`, `EventDef::new`) or writes single cells.

### A compatible change

An addition is compatible: a stack, a table, a series, an event type, a setting, a command, a field of a shape, or an
option at the end of a list of options.

1. Add the item.
2. Raise the minor version: `Family::new("ais", 1, 1)`.
3. Say in which version the item appeared: `.added(1, 1, &["shape.ShipRow.draught_m", "ais.table.fences"])`. A path is
   `shape.<Struct>` or `shape.<Struct>.<key>`, `<stack>`, `<stack>.fields.<Struct>`, `<stack>.table.<key>`,
   `<stack>.series.<key>`, `<stack>.raster.<key>`, `<stack>.event.<type>`, `<stack>.setting.<key>`,
   `<stack>.command.<key>`, `<stack>.component.<name>.fields.<Struct>`, `<stack>.component.<name>.dropped` or
   `<stack>.kept.<entry>`.
4. Write the files again.

For a version that adds whole stacks, do not list the paths by hand: `.added_in(1, 1, |f| f.stack(..).stack(..))` marks
every item that the closure brings as added in that version.

A new field goes to the end of the C struct, whatever its place in the Rust struct. A program that was built with the
older header reads the members that it knows.

### A breaking change

A removal, a new name, another type or unit, another row shape, another list of arguments, and a field that becomes
optional or stops being optional are breaking.

1. Make the change.
2. Raise the major version and set the minor version to 0: `Family::new("ais", 2, 0)`. Remove the `.added(..)` lines:
   every item is of the new version.
3. Write the files again.

A consumer compares `SDRKIT_<FAMILY>_VERSION_MAJOR` of its header with `sdrkit_family_version()` of the library.

## Building with fewer families

Each family is a cargo feature of `sdrkit-host-catalog` and of the C library `sdrkit-host-ffi`, all on by default.

```sh
cargo build --release -p sdrkit-host-ffi --no-default-features --features adsb,fm
```

The library then holds only these stacks. `sdrkit_family_version()` returns `SDRKIT_E_NO_SUCH_KEY` for a family that
is not in the library. Install only the headers of the families that you build: the header of another family compiles,
and its functions return `SDRKIT_E_NO_SUCH_KEY` or `SDRKIT_E_WRONG_HANDLE`.

## Moving a stack crate to a description

Work in the stack crate only. No other crate needs an edit.

1. Record what a generic consumer sees now. Add `sdrkit-dev-interface` to the dev-dependencies, add
   `tests/interface.rs` with `sdrkit_dev_interface::declarations::check(&descriptors(), &path)`, and write
   `tests/declarations.txt` once: `SDRKIT_WRITE_DECLARATIONS=1 cargo test -p sdrkit-stack-<family> --test interface`.
   Do not write this file again during the move.
2. Add `src/typed.rs` and `pub mod typed;`. For each list of `Field` constants, write a struct with the same keys,
   labels, units and order. For each `TableDef` and `EventDef`, write a `Table` and an `EventKind`. For each command
   with arguments, write an arguments struct and a `CommandKind`. Write `pub fn family() -> &'static Family`.
3. Replace each group of `set` and `opt` calls with one struct, and each `Object` with `Shaped`. Build each receive chain
   with `StackGraph::of`. A receive chain that is not a `StackGraph` calls `StackShape::difference` when it is made.
4. Remove the constants. Run the test of step 1: `cargo test -p sdrkit-stack-<family> --test interface`. The
   declarations are the same as before.
5. Write the files with the command of "What is written from the description", then run the check below it.

The files that appear: `tests/interface.rs`, `tests/declarations.txt` and `src/typed.rs` in the stack crate,
`registry/<family>.reg`, `crates/hosts/sdrkit-host-ffi/include/sdrkit/<family>.h` and
`crates/hosts/sdrkit-host-ffi/swift/<Family>.swift`. The pages `docs/stacks/<stack>.md` of the family change.

A file that several stacks use has the id of none of them. Name it in the description, with the stacks that use it:
`Family::new(..).shared_source("teleprinter", &["rtty", "sitor"])` for `src/teleprinter.rs` or the folder
`src/teleprinter`. The test treats the file as moved when every one of these stacks is in the description.

The test `the_keys_with_a_unit_and_a_plain_number_are_listed` of `families` prints, for each family, the fields,
columns, settings and arguments whose key ends in `_hz`, `_khz`, `_mhz`, `_s`, `_ms`, `_db`, `_dbm`, `_m`, `_km` or
`_deg` and whose kind is a plain number. It does not fail. Run it with `--nocapture` to plan the correction of a
family: a frequency is `Hertz`, a length of time is `Seconds`.

A crate can move one stack at a time. `family()` lists the stacks that have moved. The test
`a_described_stack_publishes_through_its_shapes_only` of `families` reads the source of the crate. A source file
belongs to a stack if a folder or the file of its path has the id of the stack (`src/ltr/mod.rs`, `src/ltr.rs`). The
test fails if a file of a listed stack declares a field, a table or an event type by hand or writes single cells. For
the other files it prints each part of the crate with the number of such places, as "not moved", and does not fail.
Keep the code of a stack in a folder or a file with its id, so that the test finds it. The ignored test
`a_family_publishes_through_its_shapes_only` is the gate at the end: it fails for any such place in a crate with a
description.
