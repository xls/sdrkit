//! The C header of a family: a struct for each row shape, result set, event, arguments struct and reply; an enum for
//! each field and setting with options; a mask type with one bit constant for each flag of a flag set; and a function for each table, result set, event type, command and setting.
//! Each function is a wrapper over one entry point of the base header, so the library holds no function of a family.

use crate::{
    args_structs, component_set, doubled_word, event_types, generator_setting, joined, scoped,
    shapes, snake,
};
use sdrkit_core::interface::layout::{self, CType, Head, Layout};
use sdrkit_core::interface::{EventShape, Family, Shape, StackShape};
use sdrkit_core::{ChoiceDecl, FieldDecl, FlagDecl, ValueKind};
use std::fmt::Write;

fn c_type(t: CType) -> &'static str {
    match t {
        CType::Bool => "bool",
        CType::Enum => "int32_t",
        CType::Flags => "uint64_t",
        CType::I64 => "int64_t",
        CType::F64 => "double",
        CType::Str => "sdrkit_string",
        CType::Bytes => "sdrkit_bytes",
        CType::Position => "sdrkit_position",
        CType::Variant => "sdrkit_variant",
    }
}

fn what(d: &FieldDecl, enum_name: Option<&str>) -> String {
    match d.kind {
        ValueKind::Hz => " A frequency in Hz.".into(),
        ValueKind::Time => " Seconds since 1970 (UTC).".into(),
        ValueKind::Duration => " A length of time in seconds.".into(),
        ValueKind::Choice => match enum_name {
            Some(n) => format!(" A value of `{n}`."),
            None => " The index of an option.".into(),
        },
        ValueKind::Flags => match enum_name {
            Some(n) => format!(" A mask of the `{}_*` bits.", n.to_uppercase()),
            None => " A mask of flags.".into(),
        },
        _ if d.unit.is_empty() => String::new(),
        _ => format!(" Unit: {}.", d.unit),
    }
}

/// The name of the enum of the member `key` of the struct `name`, and the start of the names of its constants. They
/// are the name of the struct and the key. A member whose key is the last word of the struct (the member `event` of
/// an event struct) has the enum `<struct>_value`, and its constants start with the name of the struct. A key that
/// begins with the last words of the struct does not say them again.
pub fn member_enum(name: &str, key: &str) -> (String, String) {
    let key = snake(key);
    if name.ends_with(&format!("_{key}")) {
        return (format!("{name}_value"), name.to_uppercase());
    }
    // The key does not say the last words of the struct again: `event_kind` of `sdrkit_same_alert_event` gives
    // `sdrkit_same_alert_event_kind`.
    let words: Vec<&str> = key.split('_').collect();
    let overlap = (1..words.len())
        .rev()
        .find(|k| name.ends_with(&format!("_{}", words[..*k].join("_"))))
        .unwrap_or(0);
    let full = format!("{name}_{}", words[overlap..].join("_"));
    (full.clone(), full.to_uppercase())
}

/// Writes the enum `name` with one constant for each option, if there is an option, and adds its names to `names`.
/// The name of each constant is `upper` and the word of the option.
fn c_enum(
    out: &mut String,
    name: &str,
    upper: &str,
    doc: &str,
    choices: &[ChoiceDecl],
    words: &[String],
    names: &mut Vec<String>,
) {
    if choices.is_empty() {
        return;
    }
    let _ = writeln!(out, "// {doc}");
    let _ = writeln!(out, "typedef enum {name} {{");
    for (i, (c, word)) in choices.iter().zip(words).enumerate() {
        let constant = format!("{upper}_{}", word.to_uppercase());
        let _ = writeln!(out, "  // {}", c.label);
        let _ = writeln!(out, "  {constant} = {i},");
        names.push(constant);
    }
    let _ = writeln!(out, "}} {name};\n");
    names.push(name.to_owned());
}

/// Writes the mask type `name` of a flag set and one bit constant for each flag, if there is a flag, and adds the
/// names to `names`. The name of each constant is `upper` and the word of the flag. Bit n is entry n of the
/// declaration, which `sdrkit_*_option` reads at run time.
fn c_flags(
    out: &mut String,
    name: &str,
    upper: &str,
    doc: &str,
    flags: &[FlagDecl],
    words: &[String],
    names: &mut Vec<String>,
) {
    if flags.is_empty() {
        return;
    }
    let _ = writeln!(out, "// {doc}");
    let _ = writeln!(out, "typedef uint64_t {name};");
    for (i, (f, word)) in flags.iter().zip(words).enumerate() {
        let constant = format!("{upper}_{}", word.to_uppercase());
        let _ = writeln!(out, "// {}: {}", f.label, f.help);
        let _ = writeln!(out, "#define {constant} (UINT64_C(1) << {i})");
        names.push(constant);
    }
    out.push('\n');
    names.push(name.to_owned());
}

/// The name of the C struct of a row, a result set, a reply or the arguments of a command, from the name of its Rust
/// struct: `sdrkit_<scope>_<rest>` (see [`scoped`]). The struct of a shared interface that `family` uses has the name
/// that it has in that interface.
pub fn struct_name(family: &Family, name: &str) -> String {
    let (scope, rest) = scoped(family.owner_of_shape(name), name);
    format!("sdrkit_{scope}_{rest}")
}

/// The name of the struct `name` in a call of the library: the key of the interface that describes it, a dot, and the
/// name.
pub fn type_key(family: &Family, name: &str) -> String {
    format!("{}.{name}", family.owner_of_shape(name).key)
}

/// The name of the C struct that holds the counts of dropped items of the decoder components of the stack `stack`.
pub fn dropped_struct_name(stack: &str) -> String {
    format!("sdrkit_{}", joined(&snake(stack), "dropped"))
}

/// The name of the C function that runs the command `key` of the stack `stack`.
pub fn command_function(stack: &str, key: &str) -> String {
    format!("sdrkit_{}", joined(&snake(stack), &snake(key)))
}

/// The words of the table `key` of the stack `stack` in a generated name, with `_` between them: the stack, then the
/// key, joined as [`joined`] does. A key that is the id of the stack or its last words is not said twice: the table
/// `events` of the stack `beacon_events` gives `beacon_events`.
pub fn table_words(stack: &str, key: &str) -> String {
    joined(&snake(stack), &snake(key))
}

/// The words of the table `key` of `stack` of `family` in a generated name. They are the words of [`table_words`],
/// unless another table of the stack has the same words. Of two such tables, the table whose words leave out a part
/// of the key keeps them, and the table whose key stands whole in its words gets the key of the family before its
/// key. The tables `stations` and `js8_stations` of the stack `js8` of the family `wsjt` give `js8_wsjt_stations` and
/// `js8_stations`. The words depend on the keys of the tables of the stack only, not on their order.
pub fn stack_table_words(family: &Family, stack: &StackShape, key: &str) -> String {
    let sid = snake(stack.id);
    let words = table_words(stack.id, key);
    let whole = |k: &str| table_words(stack.id, k) == format!("{sid}_{}", snake(k));
    let same = stack
        .tables
        .iter()
        .any(|t| t.def.key != key && table_words(stack.id, t.def.key) == words);
    if same && whole(key) {
        joined(&sid, &joined(&snake(family.key), &snake(key)))
    } else {
        words
    }
}

/// The name of the C function that reads the rows of a table with the key `key` and the words `words`:
/// `sdrkit_<words>_rows`. A key that ends in the word `rows` does not get the word again.
fn rows_function(words: &str, key: &str) -> String {
    let key_snake = snake(key);
    if key_snake == "rows" || key_snake.ends_with("_rows") {
        format!("sdrkit_{words}")
    } else {
        format!("sdrkit_{words}_rows")
    }
}

/// The name of the C function that reads the rows of the table `key` of the stack `stack`:
/// `sdrkit_<stack>_<table>_rows` (see [`table_words`]). A key that ends in the word `rows` does not get the word
/// again: the table `lv_rows` of the stack `astro` has `sdrkit_astro_lv_rows`, and the table `rows` of the stack
/// `ermes` has `sdrkit_ermes_rows`.
pub fn table_function(stack: &str, key: &str) -> String {
    rows_function(&table_words(stack, key), key)
}

/// The name of the C function that reads the rows of the table `key` of `stack` of `family`, made from the words of
/// [`stack_table_words`] as [`table_function`] makes it.
pub fn stack_table_function(family: &Family, stack: &StackShape, key: &str) -> String {
    rows_function(&stack_table_words(family, stack, key), key)
}

/// The name of the C function that reads an event of the type `e` of `family`: the name of its struct, then
/// `_read`.
pub fn event_function(family: &Family, e: &EventShape) -> String {
    format!("{}_read", event_struct_name(family, e))
}

/// The names of the C functions that read and write the setting `key` of the stack `stack`. A setting of the test
/// signal generator (`gen_<name>`) has the functions `sdrkit_<stack>_gen_get_<name>` and `sdrkit_<stack>_gen_set_<name>`.
pub fn setting_functions(stack: &str, key: &str) -> (String, String) {
    let sid = snake(stack);
    match generator_setting(key) {
        Some(rest) => (
            format!("sdrkit_{sid}_gen_get_{}", snake(rest)),
            format!("sdrkit_{sid}_gen_set_{}", snake(rest)),
        ),
        None => (
            format!("sdrkit_{sid}_get_{}", snake(key)),
            format!("sdrkit_{sid}_set_{}", snake(key)),
        ),
    }
}

/// The name of the C struct of the events of the type `e` of `family`: `sdrkit_<words>_event`, with the words of
/// [`event_words`](crate::event_words). Each event type has its own struct, and a type that stacks list with
/// several shapes has one for each shape. A type whose key ends in the word `event` does not get the word again:
/// `science.event` has `sdrkit_science_event`.
pub fn event_struct_name(family: &Family, e: &EventShape) -> String {
    let kind = crate::event_words(family, e);
    if kind == "event" || kind.ends_with("_event") {
        format!("sdrkit_{kind}")
    } else {
        format!("sdrkit_{kind}_event")
    }
}

fn members(
    out: &mut String,
    family: &Family,
    l: &Layout,
    head: Head,
    decl: impl Fn(usize) -> (FieldDecl, String),
    enums: impl Fn(&str) -> Option<String>,
) {
    for m in &l.members {
        let (d, doc) = decl(m.index);
        let name = crate::member_name_in(family, &m.key, head);
        if m.has_at.is_some() {
            let _ = writeln!(
                out,
                "  // True if `{name}` holds a value.\n  bool has_{name};"
            );
        }
        let e = enums(&m.key);
        let _ = writeln!(out, "  // {doc}{}", what(&d, e.as_deref()));
        // A flag set has its mask type.
        let ty = match &e {
            Some(mask) if m.ctype == CType::Flags => mask.as_str(),
            _ => c_type(m.ctype),
        };
        let _ = writeln!(out, "  {ty} {name};");
    }
}

/// Writes the struct `name` of `shape` after the enums of its members with options, and adds the names to `names`.
fn shape_struct(
    family: &Family,
    shape: &Shape,
    name: &str,
    head: Head,
    out: &mut String,
    names: &mut Vec<String>,
) -> Result<(), String> {
    let enum_base = name;
    let l = layout::of_shape(family.owner_of_shape(shape.name), shape, head);
    for f in shape.fields {
        let d = f.decl();
        let member = crate::member_name_in(family, &d.key, head);
        let doc = format!("The options of `{member}` of `{name}`.");
        let (enum_name, upper) = member_enum(enum_base, &member);
        let words = crate::option_words(family, &d.key, &d.choices)?;
        c_enum(out, &enum_name, &upper, &doc, &d.choices, &words, names);
        let doc = format!(
            "The flags of `{member}` of `{name}`: a mask with one bit for each flag that is set."
        );
        let words = crate::flag_words(family, &d.key, &d.flags)?;
        c_flags(out, &enum_name, &upper, &doc, &d.flags, &words, names);
    }
    let mut seen: Vec<String> = Vec::new();
    for m in &l.members {
        let member = crate::member_name_in(family, &m.key, head);
        if seen.contains(&member) {
            return Err(format!("the struct {name} has two members {member}"));
        }
        seen.push(member);
    }
    let _ = writeln!(out, "// {}", shape.doc);
    let _ = writeln!(out, "typedef struct {name} {{");
    out.push_str(
        "  // The number of bytes of this struct that the library wrote.\n  uint32_t size;\n",
    );
    match head {
        Head::Plain => {}
        Head::Row => out.push_str("  // The row key.\n  sdrkit_string key;\n"),
        Head::Event => out.push_str(
            "  // Seconds since the object started, in the time of the signal.\n  double time;\n  // The frequency of the signal in Hz.\n  double freq_hz;\n",
        ),
    }
    members(
        out,
        family,
        &l,
        head,
        |i| (shape.fields[i].decl(), shape.notes[i].doc.to_owned()),
        |key| {
            let d = shape.fields.iter().find(|f| f.key() == key)?.decl();
            (!d.choices.is_empty() || !d.flags.is_empty())
                .then(|| member_enum(enum_base, &crate::member_name_in(family, key, head)).0)
        },
    );
    let _ = writeln!(out, "}} {name};\n");
    names.push(name.to_owned());
    Ok(())
}

/// The parameter type, the suffix of the entry point and the conversion of a setting of `kind`.
fn setting_type(kind: ValueKind) -> Option<(&'static str, &'static str)> {
    Some(match kind {
        ValueKind::Bool => ("bool", "bool"),
        ValueKind::I64 => ("int64_t", "i64"),
        ValueKind::F64 | ValueKind::Hz | ValueKind::Time | ValueKind::Duration => ("double", "f64"),
        ValueKind::Text => ("sdrkit_string", "string"),
        ValueKind::Choice => ("int32_t", "enum"),
        ValueKind::Position => ("sdrkit_position", "position"),
        ValueKind::Bytes | ValueKind::Any | ValueKind::Flags => return None,
    })
}

/// The C header of `family`, for the file `include/sdrkit/<family>.h`.
///
/// # Errors
///
/// A sentence, if a shape has a field without a fixed layout, two names of the header are equal, or several stacks
/// list an event type with one shape that is not the first shape of the type.
pub fn render(family: &Family) -> Result<String, String> {
    if let Some(problem) = crate::event_shape_without_a_stack(family) {
        return Err(problem);
    }
    if let Some(problem) = crate::wrong_stop_command(family) {
        return Err(problem);
    }
    let fam = family.key;
    let upper = fam.to_uppercase();
    let mut out = String::new();
    let _ = write!(
        out,
        "/*\n\
         \x20* sdrkit: the typed interface of {subject}, version {v}. MIT licence.\n\
         \x20*\n\
         \x20* Written from the interface description of the family. Do not edit.\n\
         \x20*\n\
         \x20* Structs. Each struct starts with `size`. A call that fills a struct writes at most the size of the struct that\n\
         \x20* the caller was compiled with, and sets `size` to the number of bytes that it wrote. A struct only grows at its\n\
         \x20* end: a library that is newer than the caller fills the members that the caller knows, and a library that is\n\
         \x20* older leaves the members that it does not know at 0, with `size` below the size of the struct. The caller\n\
         \x20* sets `size` of an arguments struct to its sizeof before the call: use the *_INIT initialiser.\n\
         \x20*\n\
         \x20* Values. A member with a `has_` member before it holds a value only if the `has_` member is true. A string or\n\
         \x20* bytes member that the library writes is a view into a buffer of the handle, valid until the next call on the\n\
         \x20* same handle. A member of an enumeration holds a value of the enum that its comment names.\n\
         \x20*\n\
         \x20* Versions. The minor version rises when the family gets a new stack, table, member, setting, command or event\n\
         \x20* type. The major version rises when one is removed or renamed, or its type or unit changes.\n\
         \x20* sdrkit_family_version() returns the version of the family in the library, or SDRKIT_E_NO_SUCH_KEY if the\n\
         \x20* library was built without the family. sdrkit_object_find() with \"stack.<id>\" finds out if an engine has a\n\
         \x20* stack.\n\
         \x20*\n\
         \x20* Results. Take a snapshot with sdrkit_snapshot_read() on the handle of a session, then call the *_rows() and\n\
         \x20* *_read() functions on the same handle. Events. Take the events with sdrkit_events_take(), then call the\n\
         \x20* *_event_read() function of the type of each event; it returns SDRKIT_E_WRONG_TYPE for an event of another type.\n\
         \x20* Settings and commands. Call the *_get_*(), *_set_*() and command functions on the handle of a session of the\n\
         \x20* stack; the getters also take the handle of the stack type (\"stack.<id>\") and return the defaults. One call\n\
         \x20* sets one setting. A function returns SDRKIT_E_WRONG_HANDLE if the object of the handle does not have the\n\
         \x20* table, the fields, the setting or the command with the types of this header.\n\
         \x20*/\n\n\
         #ifndef SDRKIT_{upper}_H\n#define SDRKIT_{upper}_H\n\n#include \"../sdrkit.h\"\n{includes}\n",
        v = family.version,
        subject = if family.object {
            format!("the settings that every {fam} has")
        } else if family.shared {
            format!("the shared interface \"{fam}\", whose items the stacks of several families show")
        } else {
            format!("the stack family \"{fam}\"")
        },
        // The structs and the event functions of a shared interface are in its own header.
        includes = family
            .uses
            .iter()
            .map(|u| format!("#include \"{}.h\"\n", u.key))
            .collect::<String>(),
    );
    let _ = writeln!(
        out,
        "// The version of the family that this header describes."
    );
    let _ = writeln!(
        out,
        "#define SDRKIT_{upper}_VERSION_MAJOR {}",
        family.version.major
    );
    let _ = writeln!(
        out,
        "#define SDRKIT_{upper}_VERSION_MINOR {}\n",
        family.version.minor
    );

    let mut names: Vec<String> = Vec::new();
    for stack in &family.stacks {
        let id = stack.id;
        let sid = snake(id).to_uppercase();
        if family.object {
            continue;
        }
        if family.shared {
            let _ = writeln!(out, "// The keys of the event types.");
        } else {
            let _ = writeln!(
                out,
                "// The stack \"{id}\": its id, and the keys of its event types."
            );
            let _ = writeln!(out, "#define SDRKIT_STACK_{sid} \"{id}\"");
            names.push(format!("SDRKIT_STACK_{sid}"));
        }
        for e in &stack.events {
            let name = format!("SDRKIT_EVENT_{}", snake(e.def.kind).to_uppercase());
            if family.owner_of_event(e.def.kind).key == fam && !names.contains(&name) {
                let _ = writeln!(out, "#define {name} \"{}\"", e.def.kind);
                names.push(name);
            }
        }
        out.push('\n');
    }

    // The enums of the settings.
    let all = shapes(family);
    for stack in &family.stacks {
        for s in &stack.settings {
            let d = s.decl().field;
            let name = format!("sdrkit_{}", joined(&snake(stack.id), &snake(&d.key)));
            let doc = format!(
                "The options of the setting \"{}\" of the stack \"{}\".",
                d.key, stack.id
            );
            c_enum(
                &mut out,
                &name,
                &name.to_uppercase(),
                &doc,
                &d.choices,
                &crate::option_words(family, &d.key, &d.choices)?,
                &mut names,
            );
        }
    }

    // The structs.
    for (shape, uses) in &all {
        if uses.row && (uses.fields || uses.reply) {
            return Err(format!(
                "the shape {} is a row and a result set or a reply",
                shape.name
            ));
        }
        let name = struct_name(family, shape.name);
        if uses.row {
            shape_struct(family, shape, &name, Head::Row, &mut out, &mut names)?;
        }
        if uses.fields || uses.reply || uses.component {
            shape_struct(family, shape, &name, Head::Plain, &mut out, &mut names)?;
        }
    }
    for stack in &family.stacks {
        let decoders = stack.decoders();
        if decoders.is_empty() {
            continue;
        }
        let name = dropped_struct_name(stack.id);
        let _ = writeln!(
            out,
            "// The items that each decoder component of the stack \"{}\" dropped because it was slow. A member holds a\n// value while its decoder runs on a helper thread.",
            stack.id
        );
        let _ = writeln!(out, "typedef struct {name} {{");
        out.push_str(
            "  // The number of bytes of this struct that the library wrote.\n  uint32_t size;\n",
        );
        for d in &decoders {
            let member = snake(d);
            let _ = writeln!(
                out,
                "  // True if `{member}` holds a value.\n  bool has_{member};\n  // The items that the component \"{d}\" dropped.\n  int64_t {member};"
            );
        }
        let _ = writeln!(out, "}} {name};\n");
        names.push(name);
    }
    for e in event_types(family) {
        let name = event_struct_name(family, &e);
        shape_struct(family, &e.fields, &name, Head::Event, &mut out, &mut names)?;
    }
    for args in args_structs(family) {
        let name = struct_name(family, args.name);
        let l = layout::of_args(&args)?;
        let _ = writeln!(out, "// {}", args.doc);
        let _ = writeln!(out, "typedef struct {name} {{");
        out.push_str("  // The size of this struct: sizeof of the type. The caller sets it.\n  uint32_t size;\n");
        members(
            &mut out,
            family,
            &l,
            Head::Plain,
            |i| (args.args[i].decl().field, args.docs[i].to_owned()),
            |_| None,
        );
        let _ = writeln!(out, "}} {name};\n");
        let _ = writeln!(
            out,
            "// An arguments struct with its size set and every argument at 0."
        );
        // Every member has an initialiser, so the macro is silent under the warnings for missing ones.
        let zeros: String = l
            .members
            .iter()
            .flat_map(|m| {
                // The `has_` member of an optional argument comes before it.
                m.has_at.map(|_| ", 0").into_iter().chain([match m.ctype {
                    CType::Str | CType::Bytes | CType::Position | CType::Variant => ", {0}",
                    _ => ", 0",
                }])
            })
            .collect();
        let _ = writeln!(
            out,
            "#define {}_INIT {{sizeof({name}){zeros}}}\n",
            name.to_uppercase()
        );
        names.push(name);
    }

    // The functions.
    let mut event_functions: Vec<String> = Vec::new();
    for stack in &family.stacks {
        let sid = snake(stack.id);
        let id = stack.id;
        // A table, a command and a setting belong to a stack. A shared interface has none of its own.
        let events_only = StackShape {
            events: stack.events.clone(),
            ..StackShape::new(id)
        };
        let stack = if family.shared { &events_only } else { stack };
        for t in &stack.tables {
            let ty = struct_name(family, t.row.name);
            let f = stack_table_function(family, stack, t.def.key);
            let marks: String = crate::marks::table_sentences(t.def)
                .iter()
                .map(|s| format!("// {s}\n"))
                .collect();
            let _ = write!(
                out,
                "// Fills `rows` with the changed rows of the table \"{key}\" of the stack \"{id}\" in the snapshot of `handle`,\n\
                 // newest first, at most `capacity` rows, and fills `info` as sdrkit_snapshot_table() does.\n\
                 {marks}static inline int32_t {f}(SDRKIT_HANDLE handle,\n\
                 {pad}{ty} *rows,\n\
                 {pad}uint32_t capacity,\n\
                 {pad}sdrkit_rows_info *info) {{\n\
                 \x20 return sdrkit_typed_rows(handle, \"{shape}\", \"{key}\", rows, sizeof({ty}), capacity, info);\n\
                 }}\n\n",
                key = t.def.key,
                shape = type_key(family, t.row.name),
                pad = " ".repeat(22 + f.len()),
            );
            names.push(f);
        }
        for e in &stack.events {
            let ty = event_struct_name(family, e);
            let f = event_function(family, e);
            // Two stacks of a family can have one event type: it has one function. The function of an event type of
            // a shared interface is in the header of that interface.
            if event_functions.contains(&f) || family.owner_of_event(e.def.kind).key != fam {
                continue;
            }
            let _ = write!(
                out,
                "// Fills `event` with event `n` of the last sdrkit_events_take() on `handle`, an event of the type \"{kind}\"\n\
                 // ({label}). Returns SDRKIT_E_WRONG_TYPE if the event is of another type.\n\
                 static inline int32_t {f}(SDRKIT_HANDLE handle, uint32_t n, {ty} *event) {{\n\
                 \x20 return sdrkit_typed_event(handle, n, \"{kind}\", \"{fam}.{shape}\", event, sizeof({ty}));\n\
                 }}\n\n",
                kind = e.def.kind,
                label = e.def.label,
                shape = e.fields.name,
            );
            event_functions.push(f.clone());
            names.push(f);
        }
        for c in &stack.commands {
            let key = c.command.key;
            let f = command_function(id, key);
            let mut params = String::from("SDRKIT_HANDLE handle");
            let (mut args, mut reply) = (String::from("NULL, NULL"), String::from("NULL, NULL, 0"));
            if !c.args.args.is_empty() {
                let ty = struct_name(family, c.args.name);
                let _ = write!(params, ", const {ty} *args");
                args = format!("\"{fam}.{}\", args", c.args.name);
            }
            if !c.reply.fields.is_empty() {
                let ty = struct_name(family, c.reply.name);
                let _ = write!(params, ", {ty} *reply");
                reply = format!("\"{fam}.{}\", reply, sizeof({ty})", c.reply.name);
            }
            let help = if c.command.help.is_empty() {
                String::new()
            } else {
                format!(" {}", c.command.help)
            } + &crate::transmit_note(c.command);
            let _ = write!(
                out,
                "// Runs the command \"{key}\" ({label}) of the stack \"{id}\" on the session of `handle`.{help}\n\
                 static inline int32_t {f}({params}) {{\n\
                 \x20 return sdrkit_typed_command(handle, \"{key}\", {args}, {reply});\n\
                 }}\n\n",
                label = c.command.label,
            );
            names.push(f);
        }
        // The settings of the receiver, then those of the test signal generator in their own group.
        let (generator, receiver): (Vec<_>, Vec<_>) = stack
            .settings
            .iter()
            .map(|s| s.decl())
            .partition(|d| generator_setting(&d.field.key).is_some());
        if !generator.is_empty() {
            let _ = writeln!(
                out,
                "// The settings of the test signal generator of the stack \"{id}\" are the sdrkit_{sid}_gen_*() functions\n// after the settings of its receiver.\n"
            );
        }
        for d in receiver.iter().chain(&generator) {
            let key = d.field.key.as_ref();
            let Some((mut ty, entry)) = setting_type(d.field.kind) else {
                return Err(format!(
                    "the setting {key} of {id} has a kind without a C type"
                ));
            };
            let enum_name = format!("sdrkit_{}", joined(&sid, &snake(key)));
            let is_enum = d.field.kind == ValueKind::Choice && !d.field.choices.is_empty();
            if is_enum {
                ty = &enum_name;
            }
            let unit = what(&d.field, is_enum.then_some(enum_name.as_str()));
            let (get, set) = setting_functions(id, key);
            let of = if family.object {
                format!("every {id}")
            } else if generator_setting(key).is_some() {
                format!("the test signal generator of the stack \"{id}\"")
            } else {
                format!("the stack \"{id}\"")
            };
            let help = if d.help.is_empty() {
                String::new()
            } else {
                format!(" {}", d.help)
            };
            let _ = writeln!(
                out,
                "// The setting \"{key}\" ({}) of {of}.{unit}{help}",
                d.field.label
            );
            if is_enum {
                let _ = write!(
                    out,
                    "static inline int32_t {get}(SDRKIT_HANDLE handle, {ty} *value) {{\n\
                     \x20 int32_t index = 0;\n\
                     \x20 int32_t code = sdrkit_typed_setting_get_enum(handle, \"{key}\", &index);\n\
                     \x20 if (code == SDRKIT_OK && value) *value = ({ty})index;\n\
                     \x20 return code;\n\
                     }}\n\
                     static inline int32_t {set}(SDRKIT_HANDLE handle, {ty} value) {{\n\
                     \x20 return sdrkit_typed_setting_set_enum(handle, \"{key}\", (int32_t)value);\n\
                     }}\n\n"
                );
            } else {
                let _ = write!(
                    out,
                    "static inline int32_t {get}(SDRKIT_HANDLE handle, {ty} *value) {{\n\
                     \x20 return sdrkit_typed_setting_get_{entry}(handle, \"{key}\", value);\n\
                     }}\n\
                     static inline int32_t {set}(SDRKIT_HANDLE handle, {ty} value) {{\n\
                     \x20 return sdrkit_typed_setting_set_{entry}(handle, \"{key}\", value);\n\
                     }}\n\n"
                );
            }
            names.push(get);
            names.push(set);
        }
    }
    for (shape, uses) in &all {
        if !uses.fields {
            continue;
        }
        let ty = struct_name(family, shape.name);
        let f = format!("{ty}_read");
        let _ = write!(
            out,
            "// Fills `fields` with the result fields of the snapshot of `handle`.\n\
             static inline int32_t {f}(SDRKIT_HANDLE handle, {ty} *fields) {{\n\
             \x20 return sdrkit_typed_fields(handle, \"{fam}.{shape}\", fields, sizeof({ty}));\n\
             }}\n\n",
            shape = shape.name,
        );
        names.push(f);
    }
    for stack in &family.stacks {
        let id = stack.id;
        for c in &stack.components {
            let Some(shape) = c.shape else {
                continue;
            };
            let ty = struct_name(family, shape.name);
            let f = format!("sdrkit_{}_read", component_set(id, c.component, shape.name));
            let _ = write!(
                out,
                "// Fills `fields` with the result fields of the component \"{component}\" of the stack \"{id}\" in the snapshot of\n\
                 // `handle`. A generic consumer reads them under the keys \"{component}.<key>\".\n\
                 static inline int32_t {f}(SDRKIT_HANDLE handle, {ty} *fields) {{\n\
                 \x20 return sdrkit_typed_fields(handle, \"{shape}@{component}\", fields, sizeof({ty}));\n\
                 }}\n\n",
                component = c.component,
                shape = type_key(family, shape.name),
            );
            names.push(f);
        }
        if !stack.decoders().is_empty() {
            let ty = dropped_struct_name(id);
            let f = format!("{ty}_read");
            let _ = write!(
                out,
                "// Fills `dropped` with the counts of dropped items of the decoder components of the stack \"{id}\" in the\n\
                 // snapshot of `handle`.\n\
                 static inline int32_t {f}(SDRKIT_HANDLE handle, {ty} *dropped) {{\n\
                 \x20 return sdrkit_typed_fields(handle, \"{fam}.{id}.dropped\", dropped, sizeof({ty}));\n\
                 }}\n\n",
            );
            names.push(f);
        }
    }
    let _ = writeln!(out, "#endif");

    names.sort();
    if let Some(w) = names.windows(2).find(|w| w[0] == w[1]) {
        return Err(format!("the header has the name {} twice", w[0]));
    }
    // An option can have the last word of its enum (the option "stereo" of the setting "stereo"): the names of the
    // constants are not in this check.
    for n in names.iter().filter(|n| !n.starts_with("SDRKIT_")) {
        if let Some(word) = doubled_word(n) {
            return Err(format!("the name {n} has the word {word} twice in a row"));
        }
    }
    Ok(out)
}
