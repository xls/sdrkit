//! What is written from the interface description of a stack family ([`sdrkit_core::interface::Family`]): the registry
//! record with the check of a change ([`registry`]), the C header ([`c_header`]) and the Swift file ([`swift`]).
//! [`base`] writes the initialisers of the structs of the base header. [`declarations`] writes what a generic consumer reads from a stack, and [`sources`] makes sure that a stack crate
//! with a description writes no single cell.
//!
//! [`check_file`] compares a written file with the text that the description gives now. With the environment variable
//! `SDRKIT_WRITE_INTERFACE` set, it writes the file.

#![warn(missing_docs)]

pub mod base;
pub mod c_header;
pub mod declarations;
pub mod marks;
pub mod registry;
pub mod sources;
pub mod swift;
pub mod units;

use sdrkit_core::interface::{ArgShape, EventShape, Family, Shape};
use sdrkit_core::{FieldDecl, ValueKind};
use std::path::Path;

/// `name` in lower case with `_` between its words: `AircraftRow` gives `aircraft_row`, `adsb.aircraft` gives
/// `adsb_aircraft`, and `-1` gives `minus_1`.
pub fn snake(name: &str) -> String {
    // A key that is a negative number keeps its sign as a word.
    let mut out = String::new();
    let name = match name.strip_prefix('-') {
        Some(rest) => {
            out.push_str("minus_");
            rest
        }
        None => name,
    };
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out
}

/// The name of the struct member of the field `key` in a struct of `family`: the key, without the key of the family
/// and a dot before it. The field `session.state` of the session has the member `state`.
///
/// A key that is a reserved word of C or C++ gets `_` after it: the column `default` has the member `default_`, and
/// the column `class` has the member `class_`.
pub fn member_name<'a>(family: &Family, key: &'a str) -> &'a str {
    let name = key
        .strip_prefix(family.key)
        .and_then(|k| k.strip_prefix('.'))
        .unwrap_or(key);
    RESERVED
        .iter()
        .find(|(word, _)| *word == name)
        .map_or(name, |(_, member)| member)
}

/// The name of the struct member of the field `key` in a struct of `family` with the head `head`: the name of
/// [`member_name`], with `_` after it if a member of the head has this name. The column `key` of a row is the member
/// `key_`, after the row key `key`. A column `time` or `freq_hz` of an event, and a field `size` of any struct, have
/// `_` after them too.
pub fn member_name_in(
    family: &Family,
    key: &str,
    head: sdrkit_core::interface::layout::Head,
) -> String {
    use sdrkit_core::interface::layout::Head;
    let name = member_name(family, key);
    let taken: &[&str] = match head {
        Head::Plain => &["size"],
        Head::Row => &["size", "key"],
        Head::Event => &["size", "time", "freq_hz"],
    };
    if taken.contains(&name) {
        format!("{name}_")
    } else {
        name.to_owned()
    }
}

/// The reserved words of C and of C++ (to C++20), which cannot be the name of a struct member or a parameter, each
/// with the name that it has in a generated form.
const RESERVED: [(&str, &str); 93] = [
    ("auto", "auto_"),
    ("break", "break_"),
    ("case", "case_"),
    ("char", "char_"),
    ("const", "const_"),
    ("continue", "continue_"),
    ("default", "default_"),
    ("do", "do_"),
    ("double", "double_"),
    ("else", "else_"),
    ("enum", "enum_"),
    ("extern", "extern_"),
    ("float", "float_"),
    ("for", "for_"),
    ("goto", "goto_"),
    ("if", "if_"),
    ("inline", "inline_"),
    ("int", "int_"),
    ("long", "long_"),
    ("register", "register_"),
    ("restrict", "restrict_"),
    ("return", "return_"),
    ("short", "short_"),
    ("signed", "signed_"),
    ("sizeof", "sizeof_"),
    ("static", "static_"),
    ("struct", "struct_"),
    ("switch", "switch_"),
    ("typedef", "typedef_"),
    ("union", "union_"),
    ("unsigned", "unsigned_"),
    ("void", "void_"),
    ("volatile", "volatile_"),
    ("while", "while_"),
    ("alignas", "alignas_"),
    ("alignof", "alignof_"),
    ("and", "and_"),
    ("and_eq", "and_eq_"),
    ("asm", "asm_"),
    ("bitand", "bitand_"),
    ("bitor", "bitor_"),
    ("bool", "bool_"),
    ("catch", "catch_"),
    ("char16_t", "char16_t_"),
    ("char32_t", "char32_t_"),
    ("char8_t", "char8_t_"),
    ("class", "class_"),
    ("co_await", "co_await_"),
    ("co_return", "co_return_"),
    ("co_yield", "co_yield_"),
    ("compl", "compl_"),
    ("concept", "concept_"),
    ("const_cast", "const_cast_"),
    ("consteval", "consteval_"),
    ("constexpr", "constexpr_"),
    ("constinit", "constinit_"),
    ("decltype", "decltype_"),
    ("delete", "delete_"),
    ("dynamic_cast", "dynamic_cast_"),
    ("explicit", "explicit_"),
    ("export", "export_"),
    ("false", "false_"),
    ("friend", "friend_"),
    ("mutable", "mutable_"),
    ("namespace", "namespace_"),
    ("new", "new_"),
    ("noexcept", "noexcept_"),
    ("not", "not_"),
    ("not_eq", "not_eq_"),
    ("nullptr", "nullptr_"),
    ("operator", "operator_"),
    ("or", "or_"),
    ("or_eq", "or_eq_"),
    ("private", "private_"),
    ("protected", "protected_"),
    ("public", "public_"),
    ("reinterpret_cast", "reinterpret_cast_"),
    ("requires", "requires_"),
    ("static_assert", "static_assert_"),
    ("static_cast", "static_cast_"),
    ("template", "template_"),
    ("this", "this_"),
    ("thread_local", "thread_local_"),
    ("throw", "throw_"),
    ("true", "true_"),
    ("try", "try_"),
    ("typeid", "typeid_"),
    ("typename", "typename_"),
    ("using", "using_"),
    ("virtual", "virtual_"),
    ("wchar_t", "wchar_t_"),
    ("xor", "xor_"),
    ("xor_eq", "xor_eq_"),
];

/// The ids of the stacks of `family` that use the struct `name` as a result set, a table row, the arguments or the
/// reply of a command.
fn users(family: &Family, name: &str) -> Vec<&'static str> {
    family
        .stacks
        .iter()
        .filter(|s| {
            s.fields.iter().any(|f| f.name == name)
                || s.components
                    .iter()
                    .any(|c| c.shape.is_some_and(|f| f.name == name))
                || s.tables.iter().any(|t| t.row.name == name)
                || s.commands
                    .iter()
                    .any(|c| c.args.name == name || c.reply.name == name)
        })
        .map(|s| s.id)
        .collect()
}

/// The scope and the rest of the name of the struct `name` of `family`, with `_` between the words.
///
/// - If the name begins with the id of a stack of the family, the scope is that stack and the rest is the name without
///   it: `AisStationRow` of a family with the stack "ais" gives `("ais", "station_row")`.
/// - If one stack only uses the struct, the scope is that stack, so the functions of the stack and the structs that
///   they take start with the same words: `AddressedRow` of the stack "uplink1030" gives
///   `("uplink1030", "addressed_row")`.
/// - If not, the scope is the family, and a name that begins with the key of the family does not repeat it:
///   `FmFields` of the family "fm" gives `("fm", "fields")`.
pub fn scoped(family: &Family, name: &str) -> (String, String) {
    let snake_name = snake(name);
    let words: Vec<&str> = snake_name.split('_').collect();
    // The rest of the name after the words of `scope`, if the name begins with them and has more words.
    let without = |scope: &str| -> Option<String> {
        let flat = scope.replace('_', "");
        (1..words.len())
            .find(|k| words[..*k].concat() == flat)
            .map(|k| words[k..].join("_"))
    };
    for stack in &family.stacks {
        let scope = snake(stack.id);
        if let Some(rest) = without(&scope) {
            return (scope, rest);
        }
    }
    let family_key = snake(family.key);
    let rest = without(&family_key).unwrap_or(snake_name);
    match users(family, name).as_slice() {
        [one] => {
            // The rest does not repeat the last words of the scope: `AutoFields` of "dvoice-auto" gives `fields`.
            let scope = snake(one);
            let scope_words: Vec<&str> = scope.split('_').collect();
            let rest_words: Vec<&str> = rest.split('_').collect();
            let overlap = (1..rest_words.len().min(scope_words.len() + 1))
                .rev()
                .find(|k| scope_words.ends_with(&rest_words[..*k]))
                .unwrap_or(0);
            let rest = rest_words[overlap..].join("_");
            (scope, rest)
        }
        _ => (family_key, rest),
    }
}

/// `scope` and `what` joined by `_`. The first words of `what` that are the last words of `scope` are said once, and
/// the longest such overlap counts: `("modeac", "modeac")` gives `modeac`, `("fm", "fm_fields")` gives `fm_fields`,
/// `("goes_grb", "grb_products")` gives `goes_grb_products`, and `("beacon_events", "events")` gives
/// `beacon_events`.
pub fn joined(scope: &str, what: &str) -> String {
    let scope_words: Vec<&str> = scope.split('_').collect();
    let what_words: Vec<&str> = what.split('_').collect();
    let overlap = (1..=what_words.len().min(scope_words.len()))
        .rev()
        .find(|k| scope_words.ends_with(&what_words[..*k]))
        .unwrap_or(0);
    if overlap == what_words.len() {
        scope.to_owned()
    } else {
        format!("{scope}_{}", what_words[overlap..].join("_"))
    }
}

/// The word of option `c` in a generated name, in lower case with `_` between its parts. It is the key of the option.
/// An option whose key is a bare number takes the words of its label before the first bracket: the option "0" with
/// the label "Off" gives `off`, "12000" with the label "12 kHz" gives `12_khz`, and "115200" with the label
/// "115200 (Wenet)" gives `115200`. A label that is the number itself gives the number.
pub fn option_word(c: &sdrkit_core::ChoiceDecl) -> String {
    let key = c.key.as_ref();
    let number = key.strip_prefix('-').unwrap_or(key);
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return snake(key).trim_matches('_').to_owned();
    }
    let mut out = String::new();
    // A remark in brackets after the words is not a part of the name.
    let words = c.label.split(['(', '[']).next().unwrap_or_default();
    for ch in words.chars() {
        let ch = if ch == '\u{b5}' || ch == '\u{3bc}' {
            'u'
        } else {
            ch
        };
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_');
    if out.is_empty() {
        snake(key).trim_matches('_').to_owned()
    } else {
        out.to_owned()
    }
}

/// The words of the options `choices` of the field or setting `field` of `family` in the generated names, in the order
/// of the options. An option has the name that the description gives it ([`Family::option_name`]), or the word of
/// [`option_word`]: every character that is not a letter or a digit becomes `_`, and a run of `_` becomes one.
///
/// # Errors
///
/// A sentence, if a word is empty, is not made of lower case letters, digits and `_`, or stands for two options.
pub fn option_words(
    family: &Family,
    field: &str,
    choices: &[sdrkit_core::ChoiceDecl],
) -> Result<Vec<String>, String> {
    let mut words: Vec<String> = Vec::with_capacity(choices.len());
    for c in choices {
        let given = family
            .option_names
            .iter()
            .find(|(f, o, _)| *f == field && *o == c.key);
        let word = given.map_or_else(|| option_word(c), |(_, _, name)| (*name).to_owned());
        let plain = word
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_');
        if word.is_empty() || !plain || word.starts_with('_') || word.ends_with('_') {
            return Err(format!(
                "the option {} of {field} has the name \"{word}\": give it a name with option_name",
                c.key
            ));
        }
        if words.contains(&word) {
            return Err(format!(
                "two options of {field} have the name {word}: give one a name with option_name"
            ));
        }
        words.push(word);
    }
    Ok(words)
}

/// The words of the event type `e` of `family` in a generated name, with `_` between them. A key that begins
/// with the id of a stack of the interface that describes the type, or with the key of that interface, gives its own
/// words: `fm.text` gives `fm_text`. Any other key gets the id of the stack before it if one stack only lists the
/// type, and the key of the family if several do: `text` of the stacks "rtty" and "sitor" of the family "hfdata"
/// gives `hfdata_text`, and `fsk.lock` of the stack "navtex" alone gives `navtex_fsk_lock`. The words of the key that
/// are the last words of the stack or family are not said twice. The text key of the generic interface stays as it
/// is.
///
/// A stack can list a type with a shape of its own, which is not the shape that the first stack of the interface
/// lists the type with. The id of that stack then stands in place of the scope of the key: `wsjt.decode` with the
/// own shape of the stack "fsk441" of the family "wsjt" gives `fsk441_decode`.
pub fn event_words(family: &Family, e: &EventShape) -> String {
    let kind = e.def.kind;
    let owner = family.owner_of_event(kind);
    let words = snake(kind);
    let begins = |scope: &str| words == scope || words.starts_with(&format!("{scope}_"));
    // The stacks that list the type with this shape.
    let users: Vec<&str> = owner
        .stacks
        .iter()
        .filter(|s| {
            s.events
                .iter()
                .any(|x| x.def.kind == kind && x.fields.name == e.fields.name)
        })
        .map(|s| s.id)
        .collect();
    let scopes =
        || std::iter::once(snake(owner.key)).chain(owner.stacks.iter().map(|s| snake(s.id)));
    if first_shape(owner, kind).is_some_and(|name| name != e.fields.name) {
        let stack = snake(users.first().copied().unwrap_or(owner.key));
        let rest = scopes()
            .filter(|scope| begins(scope))
            .max_by_key(String::len)
            .map_or(words.as_str(), |scope| {
                words[scope.len()..].trim_start_matches('_')
            });
        return joined(&stack, rest);
    }
    if scopes().any(|scope| begins(&scope)) {
        return words;
    }
    let scope = match users.as_slice() {
        [one] => snake(one),
        _ => snake(owner.key),
    };
    // The key does not say the last words of the scope again: `survey.carrier` of "sat_survey" gives
    // `sat_survey_carrier`.
    let scope_words: Vec<&str> = scope.split('_').collect();
    let key_words: Vec<&str> = words.split('_').collect();
    let overlap = (1..key_words.len().min(scope_words.len() + 1))
        .rev()
        .find(|k| scope_words.ends_with(&key_words[..*k]))
        .unwrap_or(0);
    joined(&scope, &key_words[overlap..].join("_"))
}

/// The name of the shape that the first stack of `owner` which lists the event type `kind` lists it with.
fn first_shape(owner: &Family, kind: &str) -> Option<&'static str> {
    owner
        .stacks
        .iter()
        .flat_map(|s| &s.events)
        .find(|x| x.def.kind == kind)
        .map(|x| x.fields.name)
}

/// The first command of `family` whose stop command is wrong: a stop on a command that does not transmit, or a stop key
/// that is not a command of the same stack. `None` if there is none.
pub fn wrong_stop_command(family: &Family) -> Option<String> {
    family.stacks.iter().find_map(|s| {
        s.commands.iter().find_map(|c| {
            let stop = c.command.stopped_by?;
            if c.command.transmits.is_none() {
                return Some(format!(
                    "the command {} of the stack {} names the stop command {stop} but does not transmit",
                    c.command.key, s.id
                ));
            }
            (!s.commands.iter().any(|o| o.command.key == stop)).then(|| {
                format!(
                    "the command {} of the stack {} names the stop command {stop}, which the stack does not have",
                    c.command.key, s.id
                )
            })
        })
    })
}

/// The first event type of `family` that several stacks list with one shape which is not the shape of the first
/// stack that lists the type: such a shape has no stack to take its name from. `None` if there is none.
pub fn event_shape_without_a_stack(family: &Family) -> Option<String> {
    event_types(family).into_iter().find_map(|e| {
        let kind = e.def.kind;
        let users = family
            .stacks
            .iter()
            .filter(|s| {
                s.events
                    .iter()
                    .any(|x| x.def.kind == kind && x.fields.name == e.fields.name)
            })
            .count();
        (users > 1 && first_shape(family, kind) != Some(e.fields.name)).then(|| {
            format!(
                "several stacks list the event type {kind} with the shape {}, which is not the first shape of the type",
                e.fields.name
            )
        })
    })
}

/// The first word that stands twice in a row in `name`, whose words are separated by `_` or begin with an upper case
/// letter: `fm` in `sdrkit_fm_fm_fields` and in `FmFmFields`.
pub fn doubled_word(name: &str) -> Option<String> {
    let mut words: Vec<String> = Vec::new();
    for part in name.split('_').filter(|p| !p.is_empty()) {
        if part.chars().all(|c| !c.is_ascii_lowercase()) {
            words.push(part.to_ascii_lowercase());
        } else {
            words.extend(snake(part).split('_').map(str::to_owned));
        }
    }
    // The verb of a setting function before a key that begins with the same word is no repeat of a scope:
    // `sdrkit_ale3g_gen_set_set` sets the setting `gen_set`.
    words
        .windows(2)
        .find(|w| w[0] == w[1] && w[0] != "set" && w[0] != "get")
        .map(|w| w[0].clone())
}

/// The key of a setting of the test signal generator of a stack without its `gen_` prefix. `None` for a setting of the
/// receiver. The generated forms put the generator settings in their own group.
pub fn generator_setting(key: &str) -> Option<&str> {
    key.strip_prefix("gen_").filter(|rest| !rest.is_empty())
}

/// The event types that `family` describes itself, in the order of their first use, each once. A type that stacks
/// list with several shapes is in the list once for each shape. An event type of a shared interface that the family
/// uses is not in the list.
pub fn event_types(family: &Family) -> Vec<EventShape> {
    let mut out: Vec<EventShape> = Vec::new();
    for e in family.stacks.iter().flat_map(|s| &s.events) {
        if family.owner_of_event(e.def.kind).key == family.key
            && !out
                .iter()
                .any(|o| o.def.kind == e.def.kind && o.fields.name == e.fields.name)
        {
            out.push(*e);
        }
    }
    out
}

/// How a shape of a family is used by its stacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Uses {
    /// A table has rows of the shape.
    pub row: bool,
    /// A stack has the shape as a result set.
    pub fields: bool,
    /// An event kind has the fields of the shape.
    pub event: bool,
    /// A command has the shape as its reply.
    pub reply: bool,
    /// A stack has the shape as a result set of a named component.
    pub component: bool,
}

/// The shapes that `family` describes itself, in the order of their first use, each once, with how the stacks use
/// it. A shape of a shared interface that the family uses is not in the list.
pub fn shapes(family: &Family) -> Vec<(Shape, Uses)> {
    let mut out: Vec<(Shape, Uses)> = Vec::new();
    let mut note = |s: Shape, mark: fn(&mut Uses)| {
        if family.owner_of_shape(s.name).key != family.key {
            return;
        }
        match out.iter_mut().find(|(o, _)| o.name == s.name) {
            Some((_, u)) => mark(u),
            None => {
                let mut u = Uses::default();
                mark(&mut u);
                out.push((s, u));
            }
        }
    };
    for stack in &family.stacks {
        for f in &stack.fields {
            note(*f, |u| u.fields = true);
        }
        for c in &stack.components {
            if let Some(f) = c.shape {
                note(f, |u| u.component = true);
            }
        }
        for t in &stack.tables {
            note(t.row, |u| u.row = true);
        }
        for e in &stack.events {
            note(e.fields, |u| u.event = true);
        }
        for c in &stack.commands {
            if !c.reply.fields.is_empty() {
                note(c.reply, |u| u.reply = true);
            }
        }
    }
    out
}

/// The words of the generated names of the result set `shape` of the component `component` of the stack `stack`, with
/// `_` between them: the stack, the component, and the name of the struct without the words of the component at its
/// start. `EgcFields` of the component "egc" of the stack "inmarsat" gives `inmarsat_egc_fields`.
pub fn component_set(stack: &str, component: &str, shape: &str) -> String {
    let (component, shape) = (snake(component), snake(shape));
    let rest = shape
        .strip_prefix(&format!("{component}_"))
        .unwrap_or(&shape);
    joined(&snake(stack), &joined(&component, rest))
}

/// The arguments structs of the commands of `family` in the order of their first use, each once.
pub fn args_structs(family: &Family) -> Vec<ArgShape> {
    let mut out: Vec<ArgShape> = Vec::new();
    for c in family.stacks.iter().flat_map(|s| &s.commands) {
        if !c.args.args.is_empty() && !out.iter().any(|a| a.name == c.args.name) {
            out.push(c.args);
        }
    }
    out
}

/// The name of the type of `f` in the registry record: the kind, with the option keys of a choice in brackets.
pub fn type_name(f: &FieldDecl) -> String {
    match f.kind {
        ValueKind::Bool => "bool".into(),
        ValueKind::I64 => "i64".into(),
        ValueKind::F64 => "f64".into(),
        ValueKind::Text => "text".into(),
        ValueKind::Hz => "hz".into(),
        ValueKind::Time => "time".into(),
        ValueKind::Duration => "duration".into(),
        ValueKind::Position => "position".into(),
        ValueKind::Bytes => "bytes".into(),
        ValueKind::Any => "any".into(),
        ValueKind::Choice => {
            let keys: Vec<&str> = f.choices.iter().map(|c| c.key.as_ref()).collect();
            format!("choice({})", keys.join(","))
        }
    }
}

/// The sentences that the comment of a command that transmits ends with, or an empty text for one that does not. A
/// transmission that lasts names the command that stops it.
pub fn transmit_note(command: &sdrkit_core::Command) -> String {
    command.transmits.map_or_else(String::new, |c| {
        let stop = command.stopped_by.map_or_else(String::new, |k| {
            format!(" The transmission lasts until the command \"{k}\" stops it.")
        });
        format!(
            " It transmits ({}). While the transmitter of the session is off it returns SDRKIT_E_TRANSMIT_OFF and sends nothing.{stop}",
            sdrkit_core::Choice::key(c)
        )
    })
}

/// True if a run with `SDRKIT_WRITE_INTERFACE` set writes the file at `path`: every file, or with `SDRKIT_WRITE_ONLY`
/// set (parts of paths with a comma between them) only a file whose path contains one of the parts.
pub fn writes(path: &Path) -> bool {
    if std::env::var_os("SDRKIT_WRITE_INTERFACE").is_none() {
        return false;
    }
    let Ok(only) = std::env::var("SDRKIT_WRITE_ONLY") else {
        return true;
    };
    let p = path.to_string_lossy();
    only.split(',')
        .map(str::trim)
        .any(|part| !part.is_empty() && p.contains(part))
}

/// Compares the file at `path` with `text`. If [`writes`] is true for `path`, it writes `text` to the file.
///
/// # Errors
///
/// A sentence that says what to do, if the file differs from `text` or cannot be read or written.
pub fn check_file(path: &Path, text: &str) -> Result<(), String> {
    if writes(path) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        return std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()));
    }
    let now = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if now == text {
        return Ok(());
    }
    let line = now
        .lines()
        .zip(text.lines())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| now.lines().count().min(text.lines().count()));
    Err(format!(
        "{} differs from the description at line {}. Run the test with SDRKIT_WRITE_INTERFACE=1 to write it.",
        path.display(),
        line + 1
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::interface::StackShape;
    use sdrkit_core::ChoiceDecl;

    fn marine() -> Family {
        Family::new("marine", 1, 0)
            .stack(StackShape::new("ais"))
            .stack(StackShape::new("marine"))
            .stack(StackShape::new("modeac"))
    }

    /// A stop command is a command of the same stack, on a command that transmits; the note of the command names it.
    #[test]
    fn a_stop_command_belongs_to_the_stack_and_to_a_transmission() {
        use sdrkit_core::{Command, TxContent};
        static HALT: Command = Command::new("halt", "Halt");
        static CQ: Command = Command::new("cq", "Call CQ")
            .transmits(TxContent::Text)
            .stopped_by("halt");
        static CLEAR: Command = Command::new("clear", "Clear").stopped_by("halt");
        let ok = Family::new("air", 1, 0).stack(StackShape::new("air").command(&CQ).command(&HALT));
        assert_eq!(wrong_stop_command(&ok), None);
        assert!(transmit_note(&CQ).ends_with("until the command \"halt\" stops it."));
        let alone = Family::new("air", 1, 0).stack(StackShape::new("air").command(&CQ));
        assert!(wrong_stop_command(&alone).unwrap().contains("does not have"));
        let quiet = Family::new("air", 1, 0).stack(StackShape::new("air").command(&CLEAR).command(&HALT));
        assert!(wrong_stop_command(&quiet).unwrap().contains("does not transmit"));
        assert!(c_header::render(&alone).is_err());
    }

    #[test]
    fn two_tables_of_a_stack_with_the_same_words_get_two_names() {
        #[derive(sdrkit_core::interface::Fields)]
        #[allow(dead_code)]
        struct StationRow {
            /// The callsign.
            #[field(label = "Callsign")]
            call: i64,
        }
        use sdrkit_core::interface::Table;
        static STATIONS: Table<StationRow> = Table::new("stations", "Stations", 10);
        static JS8_STATIONS: Table<StationRow> = Table::new("js8_stations", "JS8 stations", 10);
        static JS8_FRAMES: Table<StationRow> = Table::new("js8_frames", "JS8 frames", 10);
        let names = |stack: StackShape| {
            let family = Family::new("wsjt", 1, 0).stack(stack);
            let s = &family.stacks[0];
            s.tables
                .iter()
                .map(|t| c_header::stack_table_function(&family, s, t.def.key))
                .collect::<Vec<_>>()
        };
        let both = names(
            StackShape::new("js8")
                .table(&STATIONS)
                .table(&JS8_FRAMES)
                .table(&JS8_STATIONS),
        );
        assert_eq!(
            both,
            [
                "sdrkit_js8_wsjt_stations_rows",
                "sdrkit_js8_frames_rows",
                "sdrkit_js8_stations_rows"
            ]
        );
        // The order of the tables does not change the names.
        let turned = names(StackShape::new("js8").table(&JS8_STATIONS).table(&STATIONS));
        assert_eq!(
            turned,
            ["sdrkit_js8_stations_rows", "sdrkit_js8_wsjt_stations_rows"]
        );
        // Without the other table, a table keeps the plain words.
        assert_eq!(
            names(StackShape::new("js8").table(&STATIONS)),
            ["sdrkit_js8_stations_rows"]
        );
        assert_eq!(
            names(StackShape::new("ft8").table(&STATIONS).table(&JS8_STATIONS)),
            ["sdrkit_ft8_stations_rows", "sdrkit_ft8_js8_stations_rows"]
        );
    }

    #[test]
    fn a_struct_name_does_not_repeat_its_family_or_its_stack() {
        let fm = Family::new("fm", 1, 0).stack(StackShape::new("fm"));
        assert_eq!(c_header::struct_name(&fm, "FmFields"), "sdrkit_fm_fields");
        assert_eq!(c_header::struct_name(&fm, "AfRow"), "sdrkit_fm_af_row");
        let m = marine();
        assert_eq!(
            c_header::struct_name(&m, "AisStationArgs"),
            "sdrkit_ais_station_args"
        );
        assert_eq!(
            c_header::struct_name(&m, "ModeAcFields"),
            "sdrkit_modeac_fields"
        );
        assert_eq!(
            c_header::struct_name(&m, "HfFields"),
            "sdrkit_marine_hf_fields"
        );
        // A struct that one stack only uses has the words of that stack.
        #[derive(sdrkit_core::interface::Fields)]
        #[allow(dead_code)]
        struct AddressedRow {
            /// The address.
            #[field(label = "Address")]
            address: i64,
        }
        static ADDRESSED: sdrkit_core::interface::Table<AddressedRow> =
            sdrkit_core::interface::Table::new("addressed", "Addressed", 10);
        let adsb = Family::new("adsb", 1, 0)
            .stack(StackShape::new("adsb"))
            .stack(StackShape::new("uplink1030").table(&ADDRESSED));
        assert_eq!(
            c_header::struct_name(&adsb, "AddressedRow"),
            "sdrkit_uplink1030_addressed_row"
        );
        // The function of a command and the struct of its arguments start with the same words.
        assert!(c_header::command_function("ais", "select").starts_with("sdrkit_ais_"));
        assert_eq!(joined("modeac", "modeac"), "modeac");
        assert_eq!(joined("fm", "clear"), "fm_clear");
        assert_eq!(joined("fm", "fm_fields"), "fm_fields");
        assert_eq!(joined("goes_grb", "grb_products"), "goes_grb_products");
        assert_eq!(
            joined("slotted_voice", "voice_profile"),
            "slotted_voice_profile"
        );
        assert_eq!(joined("beacon_events", "events"), "beacon_events");
        assert_eq!(joined("a_b", "b_a"), "a_b_a");
    }

    #[test]
    fn a_word_twice_in_a_row_is_found() {
        assert_eq!(doubled_word("sdrkit_fm_fm_fields").as_deref(), Some("fm"));
        assert_eq!(doubled_word("FmFmFields").as_deref(), Some("fm"));
        assert_eq!(
            doubled_word("sdrkit_marine_ais_ais_row").as_deref(),
            Some("ais")
        );
        assert_eq!(doubled_word("sdrkit_fm_fields_read"), None);
        assert_eq!(doubled_word("aisStations"), None);
        assert_eq!(doubled_word("sdrkit_ale3g_gen_set_set"), None);
    }

    #[test]
    fn an_option_name_is_never_empty_or_doubled_and_the_description_can_give_one() {
        let options = [
            ChoiceDecl::new("F80T0.25Q-1B", "F80T0.25Q-1B"),
            ChoiceDecl::new("F80T0_25Q/1B", "Other"),
        ];
        let plain = Family::new("x", 1, 0);
        assert!(option_words(&plain, "bearer", &options[..1]).is_ok());
        let doubled = option_words(&plain, "bearer", &options).unwrap_err();
        assert!(doubled.contains("two options"), "{doubled}");
        let named = Family::new("x", 1, 0).option_name("bearer", "F80T0_25Q/1B", "other");
        assert_eq!(
            option_words(&named, "bearer", &options).unwrap(),
            ["f80_t0_25_q_1_b", "other"]
        );
        let empty = option_words(&plain, "s", &[ChoiceDecl::new("...", "?")]).unwrap_err();
        assert!(empty.contains("give it a name"), "{empty}");
        let bad = Family::new("x", 1, 0).option_name("s", "a", "Not Plain");
        assert!(option_words(&bad, "s", &[ChoiceDecl::new("a", "A")]).is_err());
    }

    #[test]
    fn an_option_whose_key_is_a_ratio_takes_the_name_of_the_description() {
        let options = [
            ChoiceDecl::new("4:3", "4:3"),
            ChoiceDecl::new("16:9", "16:9"),
        ];
        let plain = Family::new("x", 1, 0);
        assert_eq!(
            option_words(&plain, "aspect", &options).unwrap(),
            ["4_3", "16_9"]
        );
        let named = Family::new("x", 1, 0)
            .option_name("aspect", "4:3", "standard")
            .option_name("aspect", "16:9", "wide");
        assert_eq!(
            option_words(&named, "aspect", &options).unwrap(),
            ["standard", "wide"]
        );
    }

    #[test]
    fn an_event_type_without_the_id_of_a_stack_in_its_key_gets_one() {
        #[derive(sdrkit_core::interface::Fields)]
        #[allow(dead_code)]
        struct TextEvent {
            /// The count.
            #[field(label = "Count")]
            count: i64,
        }
        static TEXT: sdrkit_core::interface::EventKind<TextEvent> =
            sdrkit_core::interface::EventKind::new("text", "Text");
        static LOCK: sdrkit_core::interface::EventKind<TextEvent> =
            sdrkit_core::interface::EventKind::new("fsk.lock", "Lock");
        static CALL: sdrkit_core::interface::EventKind<TextEvent> =
            sdrkit_core::interface::EventKind::new("rtty.call", "Call");
        let family = Family::new("hfdata", 1, 0)
            .stack(StackShape::new("rtty").event(&TEXT).event(&CALL))
            .stack(StackShape::new("sitor").event(&TEXT))
            .stack(StackShape::new("navtex").event(&LOCK));
        // Several stacks list the type: the key of the family. One stack: its id. A key with a stack id: as it is.
        assert_eq!(event_words(&family, &(&TEXT).into()), "hfdata_text");
        assert_eq!(event_words(&family, &(&LOCK).into()), "navtex_fsk_lock");
        assert_eq!(event_words(&family, &(&CALL).into()), "rtty_call");
        static CARRIER: sdrkit_core::interface::EventKind<TextEvent> =
            sdrkit_core::interface::EventKind::new("survey.carrier", "Carrier");
        let satcom =
            Family::new("satcom", 1, 0).stack(StackShape::new("sat_survey").event(&CARRIER));
        assert_eq!(
            event_words(&satcom, &(&CARRIER).into()),
            "sat_survey_carrier"
        );
        assert_eq!(
            c_header::event_struct_name(&family, &(&TEXT).into()),
            "sdrkit_hfdata_text_event"
        );
    }

    #[test]
    fn a_stack_with_its_own_shape_for_an_event_type_has_its_own_struct() {
        #[derive(sdrkit_core::interface::Fields)]
        #[allow(dead_code)]
        struct DecodeEvent {
            /// The count.
            #[field(label = "Count")]
            count: i64,
        }
        #[derive(sdrkit_core::interface::Fields)]
        #[allow(dead_code)]
        struct PingEvent<'a> {
            /// The text.
            #[field(label = "Text")]
            text: &'a str,
        }
        static DECODE: sdrkit_core::interface::EventKind<DecodeEvent> =
            sdrkit_core::interface::EventKind::new("wsjt.decode", "Decode");
        static PING: sdrkit_core::interface::EventKind<PingEvent<'static>> =
            sdrkit_core::interface::EventKind::new("wsjt.decode", "Ping");
        let family = Family::new("wsjt", 1, 0)
            .stack(StackShape::new("ft8").event(&DECODE))
            .stack(StackShape::new("ft4").event(&DECODE))
            .stack(StackShape::new("jt6m").event(&PING));
        assert_eq!(event_types(&family).len(), 2);
        assert_eq!(
            c_header::event_struct_name(&family, &(&DECODE).into()),
            "sdrkit_wsjt_decode_event"
        );
        assert_eq!(
            c_header::event_struct_name(&family, &(&PING).into()),
            "sdrkit_jt6m_decode_event"
        );
        assert_eq!(event_shape_without_a_stack(&family), None);
        let header = c_header::render(&family).expect("the header");
        assert!(header.contains("typedef struct sdrkit_wsjt_decode_event {"));
        assert!(header.contains("typedef struct sdrkit_jt6m_decode_event {"));
        assert!(header.contains("sdrkit_jt6m_decode_event_read("));
        assert!(header.contains("\"wsjt.decode\", \"wsjt.PingEvent\""));
        assert!(swift::render(&family).is_ok());
        // A second shape that two stacks list has no stack to take its name from.
        let family = family.stack(StackShape::new("iscat").event(&PING));
        assert!(event_shape_without_a_stack(&family).is_some());
        assert!(c_header::render(&family).is_err());
    }

    #[test]
    fn a_member_with_a_reserved_word_of_c_or_cpp_as_its_key_has_an_underscore_after_it() {
        let x = Family::new("x", 1, 0);
        for word in [
            "default", "class", "private", "new", "bool", "true", "operator", "using",
        ] {
            assert_eq!(member_name(&x, word), format!("{word}_"));
        }
        assert_eq!(member_name(&x, "classes"), "classes");
        // A column with the name of a member of the head of its struct.
        use sdrkit_core::interface::layout::Head;
        assert_eq!(member_name_in(&x, "key", Head::Row), "key_");
        assert_eq!(member_name_in(&x, "key", Head::Plain), "key");
        assert_eq!(member_name_in(&x, "time", Head::Event), "time_");
        assert_eq!(member_name_in(&x, "size", Head::Plain), "size_");
    }

    #[test]
    fn an_option_with_a_number_as_its_key_takes_the_words_of_its_label() {
        let word =
            |key: &'static str, label: &'static str| option_word(&ChoiceDecl::new(key, label));
        assert_eq!(word("0", "Off"), "off");
        assert_eq!(word("1", "Short bursts"), "short_bursts");
        assert_eq!(word("12000", "12 kHz"), "12_khz");
        assert_eq!(word("50", "50 \u{b5}s"), "50_us");
        assert_eq!(word("0", "EBU Latin"), "ebu_latin");
        // A label that is the number itself gives the number.
        assert_eq!(word("60", "60"), "60");
        assert_eq!(word("115200", "115200 (Wenet)"), "115200");
        assert_eq!(word("16", "16 bits (the manual's example)"), "16_bits");
        assert_eq!(word("0", "(none)"), "0");
        assert_eq!(word("local", "Local"), "local");
        // No name ends in a bare number where the label has words.
        for (key, label) in [("0", "Off"), ("2", "All passes"), ("21", "021 only")] {
            let w = word(key, label);
            assert!(
                !w.ends_with(key) || w.chars().any(|c| c.is_ascii_alphabetic()),
                "{w}"
            );
        }
    }

    #[test]
    fn a_generator_setting_and_an_event_member_have_their_own_names() {
        assert_eq!(
            c_header::setting_functions("fm", "gen_ps"),
            (
                "sdrkit_fm_gen_get_ps".to_owned(),
                "sdrkit_fm_gen_set_ps".to_owned()
            )
        );
        assert_eq!(
            c_header::setting_functions("fm", "stereo"),
            (
                "sdrkit_fm_get_stereo".to_owned(),
                "sdrkit_fm_set_stereo".to_owned()
            )
        );
        // An event type without fields, which no stack lists.
        let event = |kind: &'static str| EventShape {
            def: Box::leak(Box::new(sdrkit_core::EventDef::new(kind, "", &[]))),
            fields: <() as sdrkit_core::interface::Fields>::SHAPE,
        };
        let fm = Family::new("fm", 1, 0).stack(StackShape::new("fm"));
        assert_eq!(
            c_header::event_struct_name(&fm, &event("fm.text")),
            "sdrkit_fm_text_event"
        );
        // A suffix is not added to a key that ends in its word, and a table key does not repeat the end of its stack.
        let science = Family::new("science", 1, 0).stack(StackShape::new("sid"));
        assert_eq!(
            c_header::event_struct_name(&science, &event("science.event")),
            "sdrkit_science_event"
        );
        assert_eq!(
            c_header::event_function(&science, &event("sid.event")),
            "sdrkit_sid_event_read"
        );
        assert_eq!(
            c_header::table_function("astro", "lv_rows"),
            "sdrkit_astro_lv_rows"
        );
        assert_eq!(
            c_header::table_function("radar_detect", "radar_rows"),
            "sdrkit_radar_detect_radar_rows"
        );
        assert_eq!(
            c_header::table_function("beacon_events", "events"),
            "sdrkit_beacon_events_rows"
        );
        assert_eq!(
            c_header::table_function("ermes", "rows"),
            "sdrkit_ermes_rows"
        );
        assert_eq!(c_header::table_function("fm", "af"), "sdrkit_fm_af_rows");
        assert_eq!(
            c_header::table_function("modeac", "modeac"),
            "sdrkit_modeac_rows"
        );
        assert_eq!(
            c_header::member_enum("sdrkit_adsb_aircraft_event", "event"),
            (
                "sdrkit_adsb_aircraft_event_value".to_owned(),
                "SDRKIT_ADSB_AIRCRAFT_EVENT".to_owned()
            )
        );
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use sdrkit_core::interface::{Arguments, CommandKind, StackShape};
    use std::sync::Arc;

    /// The arguments of `send`.
    #[derive(Arguments)]
    #[allow(dead_code)]
    struct SendArgs<'a> {
        /// The text.
        #[arg(label = "Text")]
        text: &'a str,
        /// The addressee.
        #[arg(label = "To", default = "^all")]
        to: Option<&'a str>,
        /// The packet that the text replies to.
        #[arg(label = "Reply to", default = 0, min = 0, max = 100, step = 1)]
        reply_id: Option<i64>,
        /// The payload.
        #[arg(label = "Payload")]
        payload: Arc<[u8]>,
    }

    static SEND: CommandKind<SendArgs<'static>> = CommandKind::new("send", "Send");

    fn mesh() -> Family {
        Family::new("mesh", 1, 0).stack(StackShape::new("mesh").command(&SEND))
    }

    /// An optional argument has a `has_` member in C and is an optional parameter in Swift, and a bytes argument is an
    /// array of bytes in Swift. With `SDRKIT_RENDER_TO` set to a folder, the two files are written there, for a
    /// compiler.
    #[test]
    fn a_command_with_optional_and_bytes_arguments_has_a_c_and_a_swift_form() {
        let header = c_header::render(&mesh()).expect("the header");
        assert!(header.contains("  bool has_to;\n"), "{header}");
        assert!(header.contains("  bool has_reply_id;\n"));
        assert!(!header.contains("has_text") && !header.contains("has_payload"));
        assert!(header.contains(
            "#define SDRKIT_MESH_SEND_ARGS_INIT {sizeof(sdrkit_mesh_send_args), {0}, 0, {0}, 0, 0, {0}}"
        ));
        let file = swift::render(&mesh()).expect("the Swift file");
        assert!(
            file.contains(
                "text: String, to: String? = nil, replyId: Int64? = nil, payload: [UInt8]"
            ),
            "{file}"
        );
        assert!(file.contains("args.has_to = to != nil"));
        if let Some(dir) = std::env::var_os("SDRKIT_RENDER_TO") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join("mesh.h"), header).expect("the header is written");
            std::fs::write(dir.join("Mesh.swift"), file).expect("the Swift file is written");
        }
    }
}

#[cfg(test)]
mod component_tests {
    use super::*;
    use sdrkit_core::interface::{EventKind, Fields, StackShape, Table};
    use std::sync::OnceLock;

    /// The counts of the message decoder.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct EgcFields {
        /// The frames with an error.
        #[field(label = "Frames bad")]
        frames_bad: i64,
    }

    /// A message.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct EgcMessageRow<'a> {
        /// The text.
        #[field(label = "Text")]
        text: &'a str,
    }

    /// A tone heard.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct ToneFields {
        /// The frequency of the tone.
        #[field(label = "Tone", unit = "Hz")]
        tone_hz: f64,
    }

    static MESSAGES: Table<EgcMessageRow<'static>> = Table::new("messages", "Messages", 150);
    static ROWS: Table<EgcMessageRow<'static>> = Table::new("rows", "Rows", 150);
    static TONE: EventKind<ToneFields> = EventKind::new("tones.tone", "Tone");

    fn tones() -> &'static Family {
        static SHARED: OnceLock<Family> = OnceLock::new();
        SHARED.get_or_init(|| {
            Family::shared("tones", 1, 0).stack(StackShape::new("tones").event(&TONE))
        })
    }

    fn satcom() -> Family {
        Family::new("satcom", 1, 0).uses(tones()).stack(
            StackShape::new("inmarsat")
                .decoder("receiver")
                .component("egc", |c| {
                    c.fields::<EgcFields>().dropped().table(&MESSAGES)
                })
                .table(&ROWS)
                .event(&TONE)
                .kept(&["rows"]),
        )
    }

    /// The results of a named component have a struct and a function with the names of the stack and the component,
    /// the decoders of a stack have one struct with their counts, and a table with the key `rows` does not repeat the
    /// word. With `SDRKIT_RENDER_TO` set to a folder, the files are written there, for a compiler.
    #[test]
    fn a_component_set_a_component_table_and_the_dropped_counts_have_typed_forms() {
        let header = c_header::render(&satcom()).expect("the header");
        for text in [
            "typedef struct sdrkit_inmarsat_egc_fields {",
            "static inline int32_t sdrkit_inmarsat_egc_fields_read(SDRKIT_HANDLE handle, sdrkit_inmarsat_egc_fields *fields)",
            "sdrkit_typed_fields(handle, \"satcom.EgcFields@egc\", fields, sizeof(sdrkit_inmarsat_egc_fields))",
            "static inline int32_t sdrkit_inmarsat_egc_messages_rows(",
            "sdrkit_typed_rows(handle, \"satcom.EgcMessageRow\", \"egc.messages\"",
            "static inline int32_t sdrkit_inmarsat_rows(",
            "typedef struct sdrkit_inmarsat_dropped {",
            "  bool has_receiver;\n",
            "  int64_t egc;\n",
            "sdrkit_typed_fields(handle, \"satcom.inmarsat.dropped\", dropped, sizeof(sdrkit_inmarsat_dropped))",
        ] {
            assert!(header.contains(text), "{text}\n{header}");
        }
        let file = swift::render(&satcom()).expect("the Swift file");
        assert!(file.contains("public enum SatcomComponents {"), "{file}");
        assert!(file.contains("public static func inmarsatEgcFields(_ handle: SDRKIT_HANDLE)"));
        assert!(file.contains("public struct InmarsatDropped {"));
        assert!(file.contains("    public var receiver: Int64?"));
        let record = registry::render(&satcom());
        for line in [
            "uses.tones major=1 since=1.0",
            "inmarsat.component.receiver.dropped since=1.0",
            "inmarsat.component.egc.fields.EgcFields since=1.0",
            "inmarsat.table.egc.messages row=EgcMessageRow cap=150 since=1.0",
            "inmarsat.kept.rows since=1.0",
        ] {
            assert!(record.lines().any(|l| l == line), "{line}\n{record}");
        }
        if let Some(dir) = std::env::var_os("SDRKIT_RENDER_TO") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join("satcom.h"), header).expect("the header is written");
            std::fs::write(dir.join("Satcom.swift"), file).expect("the Swift file is written");
        }
    }

    /// The structs and the functions of a shared interface are in its own files. A family that uses it includes its
    /// header and writes none of them again, and its record names the interface.
    #[test]
    fn an_item_of_a_shared_interface_is_written_once() {
        let own = c_header::render(tones()).expect("the header of the shared interface");
        assert!(
            own.contains("#define SDRKIT_EVENT_TONES_TONE \"tones.tone\""),
            "{own}"
        );
        assert!(own.contains("typedef struct sdrkit_tones_tone_event {"));
        assert!(own.contains("sdrkit_typed_event(handle, n, \"tones.tone\", \"tones.ToneFields\""));
        assert!(!own.contains("SDRKIT_STACK_"));
        let user = c_header::render(&satcom()).expect("the header of the family");
        assert!(
            user.contains("#include \"../sdrkit.h\"\n#include \"tones.h\"\n"),
            "{user}"
        );
        assert!(!user.contains("sdrkit_tones_tone_event"));
        assert!(!user.contains("SDRKIT_EVENT_TONES_TONE"));
        let file = swift::render(&satcom()).expect("the Swift file");
        assert!(!file.contains("TonesToneEvent"));
        assert!(swift::render(tones())
            .expect("the Swift file")
            .contains("public struct TonesToneEvent {"));
        let record = registry::render(&satcom());
        assert!(!record.contains("shape.ToneFields"), "{record}");
        assert!(record.contains("inmarsat.event.tones.tone fields=ToneFields since=1.0"));
        assert!(registry::render(tones()).contains("shape.ToneFields.tone_hz"));
        if let Some(dir) = std::env::var_os("SDRKIT_RENDER_TO") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::write(dir.join("tones.h"), own).expect("the header is written");
        }
    }
}
