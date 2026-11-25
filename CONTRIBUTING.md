# Contributing to sdrkit

This page states the design of sdrkit and the rules that a change must follow. Read it before you write code.

Three other pages hold the detail:

- [ARCHITECTURE.md](ARCHITECTURE.md) describes each part of the library: objects, streams, graphs, drivers, the engine.
- [docs/stack-interface.md](docs/stack-interface.md) is the procedure to describe the interface of a stack.
- [docs/testing.md](docs/testing.md) describes test signals, recordings and benchmarks.

## What sdrkit is

sdrkit is a library. It reads samples from a radio, a recording or the application. It runs the samples through
stacks and returns audio, pictures, media packets and decoded data. An application uses it through a C interface, a
Swift layer or the Rust crates.

Terms that this page uses:

- A **stack** is the receive chain of one technology (FM, ADS-B, DVB-T2), with its settings and its results.
- A **family** is the group of stacks in one crate, `crates/stacks/sdrkit-stack-<family>`.
- A **component** is one processing step with typed input and output ports: a filter, a demodulator, a decoder.
- A **graph** is a set of components and the connections between their ports. A stack is a graph.
- A **session** is one running stack on one input.
- A **host** is the application that calls the library.
- A **radio** is the hardware that delivers samples: an SDR board, a dongle, or a network server of one.
- A **driver** is the code that talks to one kind of radio. It sets frequency, rate and gain and brings samples in. It knows nothing about any signal standard.
- A **receive chain** is the signal processing inside a stack, from samples to decoded data.

The word "receiver" is not used in these pages for a stack or for a radio. In code, a component named `receiver` is the receive chain of its stack.

## The priorities

When two goals conflict, the earlier one in this list wins:

1. A clean and stable interface for the host.
2. Performance.
3. No duplicated code.
4. New technology is added through registries, without a change to the framework.

## The layers

A crate is `crates/<group>/sdrkit-<group>-<name>`. A group depends only on the groups above it in this table.

| Group | Contents |
|---|---|
| `framework` | Settings, results, events, commands, streams, components, graphs, the contract of a stack, the macros |
| `util` | Bit and byte tools, text codes, identity tables, geodesy, typed publishing |
| `algo` | Filters, transforms, error correction, modems, ciphers, channel models |
| `proto` | Protocols that more than one stack uses |
| `codec` | Audio, speech and picture decoders |
| `svc` | Services that work on decoded data: recorder, scanner, message store |
| `stacks` | One crate per family of stacks |
| `drivers` | One crate per kind of radio or network server, and the USB transport |
| `serve` | Serving the samples of a radio to network clients: the hub with its queues and one crate per protocol |
| `hosts` | The engine, the catalog of stacks, the C interface, the command line tool, the terminal application |
| `helpers` | Libraries for a host that are not part of the framework, such as the reader of data files |
| `dev` | Tools for tests and the generator of the interface files |

Two rules keep the layers apart:

- `sdrkit-host-catalog` is the only crate that names every stack crate. The engine depends on no stack crate.
- A stack crate depends on no other stack crate. Code that two families need goes into `algo`, `proto` or `util`.

## Design rules

### 1. Shared before private

A general algorithm has one implementation, in `crates/algo` or `crates/proto`. A stack does not keep a private copy of
a filter, a synchroniser, a code or a carrier search.

If you need an algorithm that the shared crates lack, add it to the shared crate first, with tests against the vectors
of its standard. Then use it in the stack. If a shared part does not fit your case, extend the shared part.

A part in a hot path has a plain form for every target. It can also have a NEON form for 64-bit ARM. A test compares
the two forms bit for bit.

### 2. The framework holds no regional knowledge

The framework and the stacks contain no frequency, no channel table, no station list and no limit that depends on a
country. The host sets every such value as a setting.

Regional data is a file under `data/`. The helper library `sdrkit-presets` reads these files for a host that wants
them. A stack receives plain settings and does not know where a value came from.

The engine has no region setting, no plan table and no plan command, and the station holds no country. A default
frequency in a stack descriptor is a value that the standard of the signal names, or 0. Some descriptors still hold a
regional default; do not add to these. A change that sets one of them to 0 and lets the host give the value is
welcome.

A table that is part of the standard of a signal is not regional data. Code tables, interleavers and the country codes
of a message format stay in the code.

### 3. One switch for one function

A function has one setting that controls it. Transmission is the example: the engine setting `transmit_allowed` is the
only switch. A stack or a driver declares that it can transmit. It does not add a second gate.

### 4. A component does not move the radio

A stack states what it needs: a frequency, a sample rate, an offset. The engine does the tuning. A component never
writes to a radio. A stack whose rate depends on a setting, such as a channel width, marks the setting with
`.sets_rate(f)`; the engine then sets the radio to the rate of the value. One session can ask the engine to retune through a request, and the engine decides.

### 5. Replace in place

The library has no released version. Do not keep an old form beside a new one. Do not add a bridge, a converter or a
deprecated alias. Change every caller in the same change and delete the old form.

### 6. A detector is honest

A stack that only detects a signal, or measures it, says so in its status. It declares only what it measures. It does
not show a decoded value that it did not decode.

## The interface contract

This is the most important part of the design. The interface that a host sees is a contract, and the tools enforce it.

### One description, two interfaces

Each family has one description of its interface in `src/typed.rs`. The description lists, for every stack of the
family:

- result sets (the present state of the stack),
- tables and the type of their rows,
- events,
- commands and their arguments,
- settings, with unit, range, default and help text.

Each item is a Rust struct or constant with its key, label and unit. Everything else is written from the description:

| Output | For |
|---|---|
| The Rust types that the stack fills | The stack itself |
| A C header `include/sdrkit/<family>.h` | A host in C or C++ |
| A Swift file `swift/<Family>.swift` | A host in Swift |
| The declarations of the generic interface | A host that shows any stack without knowing it |
| The page `docs/stacks/<stack>.md` | A reader |
| The record `registry/<family>.reg` | The compatibility check |

So a host has two interfaces to the same data, and they cannot drift apart:

- The **typed interface** is the first choice. A host that knows its stack calls `sdrkit_fm_fields_read`,
  `sdrkit_adsb_aircraft_rows` or `sdrkit_session_set_tuned_hz`. A wrong name or a wrong type does not compile.
- The **generic interface** finds tables, columns and settings by text key at run time. It is for a host that shows
  every stack with one set of screens, such as the terminal application.

Do not write a declaration by hand, and do not edit a generated file. Change the description and run the generation
command from [docs/stack-interface.md](docs/stack-interface.md).

### A stack writes only through its types

A stack fills the struct of its result set and the structs of its rows. It cannot put a value into a table of another
type, and it cannot publish a key that its description does not list. A test in `sdrkit-host-catalog` fails if a
described stack uses the older path that writes single cells.

A graph is built from the shape of its stack with `StackGraph::of(shape, graph)`. A graph that does not match the shape
is an error when it is built, not a surprise at run time.

Some families still use the older path. Each moves to a description by the procedure in
[docs/stack-interface.md](docs/stack-interface.md). New stacks start with a description.

### A result set states the present

A stack fills its whole result set on each report. This gives two rules:

- A measurement that does not exist at this moment is absent. It does not keep its last value.
- A value that is still true, such as a decoded text or a last position, is kept by the stack in its own state and
  written again.

### A change of frequency

When the host changes the frequency of a session, the engine clears the results of every component of the stack. The
shape of a stack lists what survives with `.kept(..)`, by table, by result set or by field: a list that a scan builds,
a database of stations, a log of received messages. A measurement of the present signal is never kept. The clearing
is not a command, so a component needs no `clear` command for it.

### Names

A shipped name is frozen. These rules give a name that can stay:

- C names are lower case with the prefix `sdrkit_`: `sdrkit_<stack>_<what>`. Constants and enumerators are upper case
  with `SDRKIT_`. A handle has the type `SDRKIT_HANDLE`. A call returns `SDRKIT_OK` or an `SDRKIT_E_*` code.
- A name does not repeat a word. The generator refuses `sdrkit_fm_fm_fields`. The verb of a setting function before a
  key of the same word is the one exception (`sdrkit_<stack>_gen_set_set` for the setting `gen_set`).
- Each event type has its own struct, even when two have the same members today.
- The settings of a test signal are in their own group: `sdrkit_fm_gen_set_ps`.
- An enumerator takes its name from the label of its option, not from a bare number.
- The text key of the generic interface never changes. A test compares the keys of a family with a recorded file.

### Structs that cross the boundary

- Every struct begins with `uint32_t size`. The caller sets it with the `*_INIT` initialiser. The library reads and
  writes no more than that number of bytes. So a struct can grow at its end, and a host that was built earlier still
  works.
- A member that can be absent has a `has_` member before it.
- A string or a byte range that the library returns is a view into a buffer of the handle. The comment of the call
  states how long the view is valid.
- No JSON crosses the C interface, in either direction. Values are structs, enumerations and typed settings.

### Versions and the registry check

Each family has a version, `major.minor`. The file `registry/<family>.reg` records every item with its type, unit and
the version in which it appeared. A test compares the description with the record:

- A new stack, table, member, setting, command or event is a compatible change. Raise the minor version and list the
  new paths with `.added(major, minor, &[..])`. Optional arguments at the end of the arguments of a command, fields at
  the end of its reply (also a first reply, which adds a parameter to the typed C function) and the content of a command
  that starts to transmit are compatible too: a host built earlier keeps working.
- A removal, a rename, a change of type or unit, or a new order of the members of a struct is a breaking change. The
  test fails until the major version is raised.

The check is there to stop an accidental break. Do not delete a record to make the test pass.

### Families at build time

Each family is a cargo feature of the catalog. A host builds the library with the families it needs. Keep a family free
of dependencies on other families so that this stays possible.

## What a stack must have

A stack is complete when it has all of these:

1. A graph of components. It uses shared components where they exist.
2. A description of its interface in `src/typed.rs`.
3. A generator of its test signal. Write it from the standard of the signal, not from the receive chain. It makes the signal
   at the sample rate that it is asked for, so `synth:<stack>` locks with default settings.
4. Tests for the worked examples and vectors of the standard, and for the receive chain at stated signal levels.
5. A `verify` case and a benchmark entry.
6. A line in `sdrkit-host-catalog`.
7. No regional data in code. See design rule 2.
8. Stage boundaries and a measured cost, if the stack can need more than half a core.

A stack never creates a thread and never asks for a number of threads. It marks the edges of its graph where whole
units cross (symbols, frames, packets) with `.stage(from, to, Stage::new(name))`, and the engine decides which marks
become cuts. Do not mark an edge inside a loop: both ends of a feedback edge belong to one stage. A loop over the
items of one component (the code blocks of a frame, the channels of a bank) goes to `ctx.pool.map(items, f)`.

The cost of each stage is a line of `cost.txt` in the family crate, in calibration units per second of signal. The
test `tests/cost.rs` of the family calls `sdrkit_graph::cost::check`; run it in a release build with
`SDRKIT_WRITE_COST=1` to write the lines, and without it to compare them: the numbers are compared in a release build
only. Do not edit the file by hand. Time the inner steps of a large component
with `io.section("name")`, so that `sdrkit profile <stack>` shows where its time goes.

## Quality of a receive chain

The fixes of the past showed the same causes again and again. Check these before you report that a receive chain is weak:

- Use the shared symbol synchroniser. A bit clock that follows edges slips at low signal levels.
- Decide on the samples of the narrow channel when the signal can be below the FM threshold. A decision on
  discriminator audio fails there.
- Follow the carrier with the shared follow-up, and filter the channel around the measured carrier.
- Use coherent detection and soft decisions where the signal permits them.
- A word decoder uses every parity bit of its code.
- A success rate of zero is a fault, not a weak receive chain. Measure each stage against theory to find it.
- A page, a frame or a message is shown only after it passed the checks of its format. Count what is refused.
- A test signal adds no noise of its own when the framework adds noise.

State in the status text of a stack which parts no public document confirms.

## Tests and gates

Run these before you propose a change. Use the crates that your change touches.

```sh
cargo test -p <crate>                      # the tests of the crate
cargo clippy -p <crate> --all-targets      # no warning
cargo test -p <crate> --test interface     # the keys of a described family are the recorded ones
sdrkit verify <stack>                      # the stack on its test signal with offset and noise
sdrkit bench <stack>                       # the processor share
sdrkit profile <stack>                     # the share of each component, section and stage
cargo check --workspace
```

Rules for tests:

- Do not relax an assertion and do not widen a tolerance to make a test pass. Find the cause.
- A test that cannot pass yet is marked `#[ignore]` with an exact reason and the measured value.
- A count on noise holds over several seeds, with a bound that follows from theory. One chosen seed proves little.
- An expected value comes from what the generator sent, not from what the receive chain printed.
- Do not run a full sweep for a small change. Run the tests of the paths that changed.

A stack must stay under one processor core at its sample rate, or its status states the reason.

## Code style

- Comments are short, plain technical English at the level of a function. A comment states what the code does and the
  contract of the function. It does not tell how the work was done or what was tried.
- Use "stack" for a receive chain and "component" for a processing step. A mode of a standard, such as DVB-T 2K, keeps
  its name.
- Name an item for what it is. Do not name by number (`demod2`).
- Run a formatter only on the files that you changed.
- The workspace uses edition 2021. The file `Cargo.toml` at the root sets the lints.

## Sources and licences

sdrkit is under the MIT licence. These rules keep it so:

- Build a stack from the public standard of its signal and from documents of the bodies that publish it. Name the
  standard in the module header.
- Do not copy code from another program, whatever its licence. Do not translate the structure of another program.
- Do not add a dependency under the GPL or the LGPL. Use a library under MIT, BSD, Apache 2.0 or CC0, and list it in
  [THIRD_PARTY.md](THIRD_PARTY.md).
- Do not put a real key, password or secret into code, tests, documents or data. A key is a setting of the host.

## Before you send a change

1. The tests and gates above pass for every crate that you changed.
2. The description of the interface, the generated files and the registry record agree.
3. No old form remains beside a new one.
4. No frequency, channel table or station list is in code.
5. The commit message states what the change does and what was measured, in plain sentences.
