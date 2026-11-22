//! The Swift file of a family: one struct for each row shape, result set and event with an optional for each value
//! that can be absent, an enum for each field and setting with options, and functions that read the structs, run the
//! commands and read and write the settings. A family file imports the C module `SDRKitC` and uses the base file
//! ([`render_base`]), which every family shares.

use crate::c_header::{event_struct_name, struct_name};
use crate::{doubled_word, event_types, generator_setting, joined, scoped, shapes, snake};
use sdrkit_core::interface::layout::Head;
use sdrkit_core::interface::{Family, Shape, StackShape};
use sdrkit_core::{FieldDecl, ValueKind};
use std::fmt::Write;

/// The Swift file that the family files share: the error code, the position, the conversions of strings, and the
/// structs of the base header `header` that begin with `size`.
pub fn render_base(header: &str) -> String {
    let mut out = "// sdrkit: what the Swift files of the stack families share. MIT licence.\n\
     // Written by the interface generator. Do not edit.\n\n\
     import SDRKitC\n\n\
     /// The code of a call that failed: an `SDRKIT_E_*` constant.\n\
     public struct SDRKitCode: Error {\n\
     \x20   public let code: Int32\n\
     }\n\n\
     /// A place on the Earth.\n\
     public struct SDRKitPosition {\n\
     \x20   /// Degrees north.\n\
     \x20   public var lat: Double\n\
     \x20   /// Degrees east.\n\
     \x20   public var lon: Double\n\
     \x20   /// Metres above the WGS 84 ellipsoid.\n\
     \x20   public var altM: Double?\n\n\
     \x20   init(_ c: sdrkit_position) {\n\
     \x20       lat = c.lat\n\
     \x20       lon = c.lon\n\
     \x20       altM = c.has_alt ? c.alt_m : nil\n\
     \x20   }\n\n\
     \x20   var c: sdrkit_position {\n\
     \x20       sdrkit_position(lat: lat, lon: lon, alt_m: altM ?? 0, has_alt: altM != nil)\n\
     \x20   }\n\
     }\n\n\
     /// A value that carries its own type: a cell of a column whose rows hold values of several types.\n\
     public enum SDRKitValue {\n\
     \x20   case none\n\
     \x20   case bool(Bool)\n\
     \x20   case integer(Int64)\n\
     \x20   case number(Double)\n\
     \x20   case text(String)\n\
     \x20   case hz(Double)\n\
     \x20   case time(Double)\n\
     \x20   case duration(Double)\n\
     \x20   case position(lat: Double, lon: Double)\n\
     \x20   /// The index of an option.\n\
     \x20   case option(UInt32)\n\
     \x20   case bytes([UInt8])\n\n\
     \x20   init(_ c: sdrkit_variant) {\n\
     \x20       switch c.type {\n\
     \x20       case SDRKIT_TYPE_BOOL: self = .bool(c.flag)\n\
     \x20       case SDRKIT_TYPE_I64: self = .integer(c.integer)\n\
     \x20       case SDRKIT_TYPE_F64: self = .number(c.number)\n\
     \x20       case SDRKIT_TYPE_STRING: self = .text(sdrkitText(c.string))\n\
     \x20       case SDRKIT_TYPE_HZ: self = .hz(c.number)\n\
     \x20       case SDRKIT_TYPE_TIME: self = .time(c.number)\n\
     \x20       case SDRKIT_TYPE_DURATION: self = .duration(c.number)\n\
     \x20       case SDRKIT_TYPE_POSITION: self = .position(lat: c.lat, lon: c.lon)\n\
     \x20       case SDRKIT_TYPE_ENUM: self = .option(c.enum_index)\n\
     \x20       case SDRKIT_TYPE_BYTES: self = .bytes([UInt8](UnsafeBufferPointer(start: c.bytes.ptr, count: c.bytes.len)))\n\
     \x20       default: self = .none\n\
     \x20       }\n\
     \x20   }\n\
     }\n\n\
     func sdrkitText(_ s: sdrkit_string) -> String {\n\
     \x20   String(decoding: UnsafeRawBufferPointer(start: s.ptr, count: s.len), as: UTF8.self)\n\
     }\n\n\
     /// Calls `body` with `text` as a string of the C interface, valid during the call.\n\
     func sdrkitWithString<R>(_ text: String, _ body: (sdrkit_string) -> R) -> R {\n\
     \x20   var copy = text\n\
     \x20   return copy.withUTF8 { b in\n\
     \x20       b.withMemoryRebound(to: CChar.self) { body(sdrkit_string(ptr: $0.baseAddress, len: $0.count)) }\n\
     \x20   }\n\
     }\n\n\
     func sdrkitResult(_ code: Int32) -> Result<Void, SDRKitCode> {\n\
     \x20   code == SDRKIT_OK ? .success(()) : .failure(SDRKitCode(code: code))\n\
     }\n\n\
     /// A struct of the C interface that begins with `size`.\n\
     public protocol SDRKitSized {\n\
     \x20   init()\n\
     \x20   var size: UInt32 { get set }\n\
     }\n\n\
     extension SDRKitSized {\n\
     \x20   /// The struct with its size set and every other member 0. Pass it to every call that takes or fills the struct.\n\
     \x20   public static var sized: Self {\n\
     \x20       var value = Self()\n\
     \x20       value.size = UInt32(MemoryLayout<Self>.size)\n\
     \x20       return value\n\
     \x20   }\n\
     }\n\n"
    .to_owned();
    for s in crate::base::sized_structs(header) {
        let _ = writeln!(out, "extension {}: SDRKitSized {{}}", s.name);
    }
    out
}

/// The Swift name of the struct of the Rust struct `name` of `family`: the scope and the rest of [`scoped`].
fn type_name(family: &Family, name: &str) -> String {
    let (scope, rest) = scoped(family.owner_of_shape(name), name);
    camel(&format!("{scope}_{rest}"), true)
}

/// The names of the cases of the enum of the options of `d`.
///
/// # Errors
///
/// A sentence, if two options get one name.
fn cases(family: &Family, key: &str, d: &FieldDecl) -> Result<Vec<String>, String> {
    let names: Vec<String> = crate::option_words(family, key, &d.choices)?
        .iter()
        .map(|w| camel(w, false))
        .collect();
    for (i, n) in names.iter().enumerate() {
        if names[..i].contains(n) {
            return Err(format!("two options of {} have the Swift name {n}", d.key));
        }
    }
    Ok(names)
}

/// The Swift type of a setting or an argument of `d`, without an enum.
fn plain_type(d: &FieldDecl) -> Option<&'static str> {
    Some(match d.kind {
        ValueKind::Bool => "Bool",
        ValueKind::I64 => "Int64",
        ValueKind::F64 | ValueKind::Hz | ValueKind::Time | ValueKind::Duration => "Double",
        ValueKind::Text => "String",
        ValueKind::Choice => "Int32",
        ValueKind::Position => "SDRKitPosition",
        ValueKind::Bytes | ValueKind::Any => return None,
    })
}

/// `name` in camel case, with the first letter in upper case if `upper`.
fn camel(name: &str, upper: bool) -> String {
    let mut out = String::new();
    let mut up = upper;
    for part in snake(name).split('_').filter(|p| !p.is_empty()) {
        let mut chars = part.chars();
        if let Some(c) = chars.next() {
            out.push(if up { c.to_ascii_uppercase() } else { c });
        }
        out.push_str(chars.as_str());
        up = true;
    }
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'n');
    }
    // The words of Swift that a member cannot have as its name without back quotes.
    const WORDS: [&str; 48] = [
        "default",
        "case",
        "class",
        "struct",
        "enum",
        "func",
        "let",
        "var",
        "in",
        "is",
        "as",
        "repeat",
        "protocol",
        "private",
        "public",
        "internal",
        "fileprivate",
        "open",
        "static",
        "self",
        "switch",
        "for",
        "while",
        "if",
        "else",
        "return",
        "true",
        "false",
        "nil",
        "init",
        "deinit",
        "import",
        "extension",
        "where",
        "break",
        "continue",
        "do",
        "try",
        "catch",
        "throw",
        "guard",
        "defer",
        "super",
        "inout",
        "operator",
        "subscript",
        "typealias",
        "associatedtype",
    ];
    if WORDS.contains(&out.as_str()) {
        out = format!("`{out}`");
    }
    out
}

fn swift_type(key: &str, kind: ValueKind) -> String {
    match kind {
        ValueKind::Bool => "Bool".into(),
        ValueKind::I64 => "Int64".into(),
        ValueKind::F64 | ValueKind::Hz | ValueKind::Time | ValueKind::Duration => "Double".into(),
        ValueKind::Text => "String".into(),
        ValueKind::Bytes => "[UInt8]".into(),
        ValueKind::Position => "SDRKitPosition".into(),
        ValueKind::Choice => enum_name(key),
        ValueKind::Any => "SDRKitValue".into(),
    }
}

/// The name of the enum of the options of the member `key` inside its struct. `Type` and `Protocol` cannot be the
/// name of a nested type: the member `type` has the enum `TypeOption`.
fn enum_name(key: &str) -> String {
    let name = camel(key, true);
    if name == "Type" || name == "Protocol" {
        format!("{name}Option")
    } else {
        name
    }
}

fn convert(key: &str, kind: ValueKind, ty: &str) -> String {
    match kind {
        ValueKind::Text => format!("sdrkitText(c.{key})"),
        ValueKind::Bytes => {
            format!("[UInt8](UnsafeBufferPointer(start: c.{key}.ptr, count: c.{key}.len))")
        }
        ValueKind::Position => format!("SDRKitPosition(c.{key})"),
        ValueKind::Choice => format!("{ty}(rawValue: c.{key})"),
        ValueKind::Any => format!("SDRKitValue(c.{key})"),
        _ => format!("c.{key}"),
    }
}

/// The declaration of the field `f` with the name of its member in a struct with the head `head` as its key
/// ([`crate::member_name_in`]).
fn member_decl(family: &Family, f: &dyn sdrkit_core::Declare, head: Head) -> FieldDecl {
    let mut d = f.decl();
    d.key = crate::member_name_in(family, &d.key, head).into();
    d
}

/// The Swift file of `family`.
///
/// # Errors
///
/// A sentence, if a shape has a field without a fixed layout.
pub fn render(family: &Family) -> Result<String, String> {
    let fam = family.key;
    let prefix = camel(fam, true);
    let mut out = String::new();
    let _ = write!(
        out,
        "// sdrkit: the typed interface of {subject}, version {}. MIT licence.\n\
         // Written from the interface description of the family. Do not edit.\n\n\
         import SDRKitC\n\n\
         /// The version of the family that this file describes.\n\
         public enum {prefix}Interface {{\n\
         \x20   public static let major = {}\n\
         \x20   public static let minor = {}\n\
         }}\n\n",
        family.version,
        family.version.major,
        family.version.minor,
        subject = if family.object {
            format!("the settings that every {fam} has")
        } else if family.shared {
            format!(
                "the shared interface \"{fam}\", whose items the stacks of several families show"
            )
        } else {
            format!("the stack family \"{fam}\"")
        },
    );
    // A table, a command and a setting belong to a stack. A shared interface has none of its own.
    let events_only: Vec<StackShape> = family
        .stacks
        .iter()
        .map(|s| StackShape {
            events: s.events.clone(),
            ..StackShape::new(s.id)
        })
        .collect();
    let stacks = if family.shared {
        &events_only
    } else {
        &family.stacks
    };
    // Each form: the shape, its use, the Swift name, the C struct, and the C function that reads an event.
    let mut forms: Vec<(Shape, crate::Uses, Head, String, String, String)> = Vec::new();
    for (shape, uses) in shapes(family) {
        let name = type_name(family, shape.name);
        let c_type = struct_name(family, shape.name);
        if uses.row {
            forms.push((
                shape,
                uses,
                Head::Row,
                name.clone(),
                c_type.clone(),
                String::new(),
            ));
        }
        if uses.fields || uses.reply || uses.component {
            forms.push((shape, uses, Head::Plain, name, c_type, String::new()));
        }
    }
    for e in event_types(family) {
        forms.push((
            e.fields,
            crate::Uses::default(),
            Head::Event,
            camel(
                event_struct_name(family, &e).trim_start_matches("sdrkit_"),
                true,
            ),
            event_struct_name(family, &e),
            crate::c_header::event_function(family, &e),
        ));
    }
    let mut names: Vec<String> = Vec::new();
    for (shape, uses, head, name, c_type, event_read) in forms {
        names.push(name.clone());
        let uses = crate::Uses {
            row: head == Head::Row,
            fields: head == Head::Plain && uses.fields,
            ..Default::default()
        };
        let _ = writeln!(out, "/// {}", shape.doc);
        let _ = writeln!(out, "public struct {name} {{");
        let mut inits = Vec::new();
        for (f, _) in shape.fields.iter().zip(shape.notes) {
            let d = member_decl(family, *f, head);
            if d.kind == ValueKind::Choice {
                let _ = writeln!(out, "    public enum {}: Int32 {{", enum_name(&d.key));
                for (i, case) in cases(family, f.key(), &d)?.iter().enumerate() {
                    let _ = writeln!(out, "        case {case} = {i}");
                }
                let _ = writeln!(out, "    }}");
            }
        }
        if uses.row {
            let _ = writeln!(out, "    /// The row key.\n    public var key: String");
            inits.push("        key = sdrkitText(c.key)".to_owned());
        }
        if head == Head::Event {
            let _ = writeln!(
                out,
                "    /// Seconds since the object started, in the time of the signal.\n    public var time: Double\n    /// The frequency of the signal in Hz.\n    public var freqHz: Double"
            );
            inits.push("        time = c.time".to_owned());
            inits.push("        freqHz = c.freq_hz".to_owned());
        }
        for (f, note) in shape.fields.iter().zip(shape.notes) {
            let d = member_decl(family, *f, head);
            let ty = swift_type(&d.key, d.kind);
            // A member with the name of a member of the head (`key_` after the row key) has `Field` after its name.
            let member = if f.key() != d.key && crate::member_name(family, f.key()) != d.key {
                format!("{}Field", camel(&d.key, false))
            } else {
                camel(&d.key, false)
            };
            let value = convert(&d.key, d.kind, &ty);
            // A choice converts to an optional: an index outside the enum reads as no value.
            let optional = note.optional || d.kind == ValueKind::Choice;
            let _ = writeln!(out, "    /// {}", note.doc);
            let _ = writeln!(
                out,
                "    public var {member}: {ty}{}",
                if optional { "?" } else { "" }
            );
            inits.push(if note.optional {
                format!("        {member} = c.has_{} ? {value} : nil", d.key)
            } else {
                format!("        {member} = {value}")
            });
        }
        let _ = writeln!(
            out,
            "\n    init(_ c: {c_type}) {{\n{}\n    }}",
            inits.join("\n")
        );
        if uses.fields {
            let _ = write!(
                out,
                "\n    /// The result fields of the snapshot of `handle`, or the code of the error.\n\
                 \x20   public static func read(_ handle: SDRKIT_HANDLE) -> Result<{name}, SDRKitCode> {{\n\
                 \x20       var c = {c_type}()\n\
                 \x20       let code = {c_type}_read(handle, &c)\n\
                 \x20       return code == SDRKIT_OK ? .success({name}(c)) : .failure(SDRKitCode(code: code))\n\
                 \x20   }}\n"
            );
        }
        if head == Head::Event {
            let _ = write!(
                out,
                "\n    /// Event `n` of the last take on `handle`, or the code of the error.\n\
                 \x20   public static func read(_ handle: SDRKIT_HANDLE, _ n: UInt32) -> Result<{name}, SDRKitCode> {{\n\
                 \x20       var c = {c_type}()\n\
                 \x20       let code = {event_read}(handle, n, &c)\n\
                 \x20       return code == SDRKIT_OK ? .success({name}(c)) : .failure(SDRKitCode(code: code))\n\
                 \x20   }}\n"
            );
        }
        let _ = writeln!(out, "}}\n");
    }
    // The result sets of named components, and the counts of dropped items of the decoder components.
    let mut component_reads = String::new();
    for stack in stacks {
        for c in &stack.components {
            let Some(shape) = c.shape else {
                continue;
            };
            let name = type_name(family, shape.name);
            let c_type = struct_name(family, shape.name);
            let both = crate::component_set(stack.id, c.component, shape.name);
            let func = camel(&both, false);
            names.push(func.clone());
            let _ = write!(
                component_reads,
                "    /// The result fields of the component \"{component}\" of the stack \"{id}\" in the snapshot of `handle`.\n\
                 \x20   public static func {func}(_ handle: SDRKIT_HANDLE) -> Result<{name}, SDRKitCode> {{\n\
                 \x20       var c = {c_type}()\n\
                 \x20       let code = sdrkit_{both}_read(handle, &c)\n\
                 \x20       return code == SDRKIT_OK ? .success({name}(c)) : .failure(SDRKitCode(code: code))\n\
                 \x20   }}\n",
                component = c.component,
                id = stack.id,
            );
        }
        let decoders = stack.decoders();
        if decoders.is_empty() {
            continue;
        }
        let c_type = crate::c_header::dropped_struct_name(stack.id);
        let name = camel(c_type.trim_start_matches("sdrkit_"), true);
        names.push(name.clone());
        let _ = writeln!(
            out,
            "/// The items that each decoder component of the stack \"{}\" dropped because it was slow. A member holds a\n/// value while its decoder runs on a helper thread.\npublic struct {name} {{",
            stack.id
        );
        let mut inits = Vec::new();
        for d in &decoders {
            let (member, c_member) = (camel(d, false), snake(d));
            let _ = writeln!(
                out,
                "    /// The items that the component \"{d}\" dropped.\n    public var {member}: Int64?"
            );
            inits.push(format!(
                "        {member} = c.has_{c_member} ? c.{c_member} : nil"
            ));
        }
        let _ = write!(
            out,
            "\n    init(_ c: {c_type}) {{\n{}\n    }}\n\n\
             \x20   /// The counts in the snapshot of `handle`, or the code of the error.\n\
             \x20   public static func read(_ handle: SDRKIT_HANDLE) -> Result<{name}, SDRKitCode> {{\n\
             \x20       var c = {c_type}()\n\
             \x20       let code = {c_type}_read(handle, &c)\n\
             \x20       return code == SDRKIT_OK ? .success({name}(c)) : .failure(SDRKitCode(code: code))\n\
             \x20   }}\n}}\n\n",
            inits.join("\n")
        );
    }
    if !component_reads.is_empty() {
        let _ = write!(
            out,
            "/// The result sets of named components of the stacks of the family, each read from the snapshot of the handle of\n/// a session.\npublic enum {prefix}Components {{\n{component_reads}}}\n\n"
        );
    }
    let _ = write!(
        out,
        "/// The tables of the family, each read from the snapshot of the handle of a session.\n\
         public enum {prefix}Tables {{\n"
    );
    for stack in stacks {
        for t in &stack.tables {
            let name = type_name(family, t.row.name);
            let c_type = struct_name(family, t.row.name);
            let both = crate::c_header::stack_table_words(family, stack, t.def.key);
            let f = crate::c_header::stack_table_function(family, stack, t.def.key);
            let func = camel(&both, false);
            names.push(func.clone());
            let marks: String = crate::marks::table_sentences(t.def)
                .iter()
                .map(|s| format!("    /// {s}\n"))
                .collect();
            let _ = write!(
                out,
                "    /// The changed rows of the table \"{key}\" of the stack \"{id}\", newest first, at most `capacity` rows.\n\
                 {marks}\x20   public static func {func}(_ handle: SDRKIT_HANDLE, capacity: Int = {cap}) -> Result<[{name}], SDRKitCode> {{\n\
                 \x20       var rows = [{c_type}](repeating: {c_type}(), count: capacity)\n\
                 \x20       var info = sdrkit_rows_info.sized\n\
                 \x20       let code = {f}(handle, &rows, UInt32(capacity), &info)\n\
                 \x20       guard code == SDRKIT_OK else {{ return .failure(SDRKitCode(code: code)) }}\n\
                 \x20       return .success(rows.prefix(min(Int(info.rows), capacity)).map({name}.init))\n\
                 \x20   }}\n",
                key = t.def.key,
                id = stack.id,
                cap = t.def.cap,
            );
        }
    }
    out.push_str("}\n");

    // The commands.
    let _ = writeln!(out, "\n/// The commands of the family, each run on the handle of a session of its stack.\npublic enum {prefix}Commands {{");
    for stack in stacks {
        for c in &stack.commands {
            let both = joined(&snake(stack.id), &snake(c.command.key));
            let func = camel(&both, false);
            names.push(func.clone());
            let mut params = String::from("_ handle: SDRKIT_HANDLE");
            let mut open = String::new();
            let mut close = String::new();
            let mut sets = String::new();
            for a in c.args.args {
                let decl = a.decl();
                let optional = !decl.required;
                let mut d = decl.field;
                d.key = crate::member_name(family, &d.key).to_owned().into();
                let ty = match d.kind {
                    ValueKind::Bytes => "[UInt8]",
                    _ => plain_type(&d).ok_or_else(|| {
                        format!("the argument {} has a kind without a Swift type", d.key)
                    })?,
                };
                let member = camel(&d.key, false);
                // An optional argument is a Swift optional that is nil by default: the library then takes the default.
                if optional {
                    let _ = write!(params, ", {member}: {ty}? = nil");
                    let _ = writeln!(sets, "        args.has_{} = {member} != nil", d.key);
                } else {
                    let _ = write!(params, ", {member}: {ty}");
                }
                match d.kind {
                    ValueKind::Text => {
                        let given = if optional {
                            format!("{member} ?? \"\"")
                        } else {
                            member.clone()
                        };
                        let _ = write!(open, "sdrkitWithString({given}) {{ {}_ in ", snake(&d.key));
                        close.push_str(" }");
                        let _ = writeln!(sets, "        args.{} = {}_", d.key, snake(&d.key));
                    }
                    ValueKind::Bytes => {
                        let given = if optional {
                            format!("({member} ?? [])")
                        } else {
                            member.clone()
                        };
                        let _ = write!(
                            open,
                            "{given}.withUnsafeBufferPointer {{ {}_ in ",
                            snake(&d.key)
                        );
                        close.push_str(" }");
                        let _ = writeln!(
                            sets,
                            "        args.{} = sdrkit_bytes(ptr: {}_.baseAddress, len: {}_.count)",
                            d.key,
                            snake(&d.key),
                            snake(&d.key)
                        );
                    }
                    kind => {
                        let c = if kind == ValueKind::Position {
                            ".c"
                        } else {
                            ""
                        };
                        if optional {
                            let _ = writeln!(
                                sets,
                                "        if let given = {member} {{ args.{} = given{c} }}",
                                d.key
                            );
                        } else {
                            let _ = writeln!(sets, "        args.{} = {member}{c}", d.key);
                        }
                    }
                }
            }
            let has_args = !c.args.args.is_empty();
            let has_reply = !c.reply.fields.is_empty();
            let args_type = struct_name(family, c.args.name);
            let reply_type = struct_name(family, c.reply.name);
            let reply_name = type_name(family, c.reply.name);
            let ret = if has_reply {
                reply_name.clone()
            } else {
                "Void".into()
            };
            let _ = writeln!(
                out,
                "    /// {}. {}{}",
                c.command.label,
                c.command.help,
                crate::transmit_note(c.command)
            );
            let _ = writeln!(
                out,
                "    public static func {func}({params}) -> Result<{ret}, SDRKitCode> {{"
            );
            if has_reply {
                let _ = writeln!(out, "        var reply = {reply_type}()");
            }
            let call_args = match (has_args, has_reply) {
                (false, false) => "handle".to_owned(),
                (true, false) => "handle, &args".to_owned(),
                (false, true) => "handle, &reply".to_owned(),
                (true, true) => "handle, &args, &reply".to_owned(),
            };
            let f = format!("sdrkit_{both}");
            if has_args {
                let _ = writeln!(out, "        var args = {args_type}()\n        args.size = UInt32(MemoryLayout<{args_type}>.size)");
                let _ = writeln!(out, "        let code: Int32 = {open}{{ () -> Int32 in\n{sets}        return {f}({call_args})\n        }}(){close}");
            } else {
                let _ = writeln!(out, "        let code = {f}({call_args})");
            }
            if has_reply {
                let _ = writeln!(out, "        return code == SDRKIT_OK ? .success({reply_name}(reply)) : .failure(SDRKitCode(code: code))");
            } else {
                let _ = writeln!(out, "        return sdrkitResult(code)");
            }
            let _ = writeln!(out, "    }}");
        }
    }
    out.push_str("}\n");

    // The settings of the receivers, then those of the test signal generators in their own group.
    for generator in [false, true] {
        let has = stacks
            .iter()
            .flat_map(|s| &s.settings)
            .any(|s| generator_setting(&s.decl().field.key).is_some());
        if generator && !has {
            continue;
        }
        if generator {
            let _ = writeln!(
                out,
                "\n/// The settings of the test signal generators of the family, each read and written on the handle of a session of\n/// its stack.\npublic enum {prefix}GeneratorSettings {{"
            );
        } else {
            let _ = writeln!(
                out,
                "\n/// The settings of the family, each read and written on the handle of a session of its stack. One call sets one\n/// setting.\npublic enum {prefix}Settings {{"
            );
        }
        for stack in stacks {
            for s in &stack.settings {
                let d = s.decl().field;
                let own_key = match (generator, generator_setting(&d.key)) {
                    (true, Some(rest)) => rest.to_owned(),
                    (false, None) => d.key.to_string(),
                    _ => continue,
                };
                let ty = plain_type(&d).ok_or_else(|| {
                    format!("the setting {} has a kind without a Swift type", d.key)
                })?;
                let both = joined(&snake(stack.id), &snake(&own_key));
                let name = camel(&both, false);
                let upper = camel(&both, true);
                names.push(name.clone());
                let key = &d.key;
                let (c_ty, entry, to_swift, to_c) = match d.kind {
                    ValueKind::Bool => ("Bool", "bool", "v".to_owned(), "value".to_owned()),
                    ValueKind::I64 => ("Int64", "i64", "v".to_owned(), "value".to_owned()),
                    ValueKind::Text => (
                        "sdrkit_string",
                        "string",
                        "sdrkitText(v)".to_owned(),
                        String::new(),
                    ),
                    ValueKind::Position => (
                        "sdrkit_position",
                        "position",
                        "SDRKitPosition(v)".to_owned(),
                        "value.c".to_owned(),
                    ),
                    ValueKind::Choice => ("Int32", "enum", "v".to_owned(), "value".to_owned()),
                    _ => ("Double", "f64", "v".to_owned(), "value".to_owned()),
                };
                let mut swift_ty = ty.to_owned();
                let (mut to_swift, mut to_c) = (to_swift, to_c);
                if d.kind == ValueKind::Choice && !d.choices.is_empty() {
                    swift_ty = upper.clone();
                    let _ = writeln!(out, "    /// The options of the setting \"{key}\" of the stack \"{}\".\n    public enum {upper}: Int32 {{", stack.id);
                    for (i, case) in cases(family, &d.key, &d)?.iter().enumerate() {
                        let _ = writeln!(out, "        case {case} = {i}");
                    }
                    let _ = writeln!(out, "    }}");
                    to_swift = format!("{upper}(rawValue: v) ?? {upper}(rawValue: 0)!");
                    to_c = "value.rawValue".into();
                }
                let init = match d.kind {
                    ValueKind::Bool => "false",
                    ValueKind::Text => "sdrkit_string()",
                    ValueKind::Position => "sdrkit_position()",
                    _ => "0",
                };
                let _ = write!(
                    out,
                    "    /// {label}.\n\
                     \x20   public static func {name}(_ handle: SDRKIT_HANDLE) -> Result<{swift_ty}, SDRKitCode> {{\n\
                     \x20       var v: {c_ty} = {init}\n\
                     \x20       let code = sdrkit_typed_setting_get_{entry}(handle, \"{key}\", &v)\n\
                     \x20       return code == SDRKIT_OK ? .success({to_swift}) : .failure(SDRKitCode(code: code))\n\
                     \x20   }}\n",
                    label = d.label,
                );
                let call = if d.kind == ValueKind::Text {
                    format!("sdrkitWithString(value) {{ sdrkit_typed_setting_set_string(handle, \"{key}\", $0) }}")
                } else {
                    format!("sdrkit_typed_setting_set_{entry}(handle, \"{key}\", {to_c})")
                };
                let _ = write!(
                    out,
                    "    public static func set{upper}(_ handle: SDRKIT_HANDLE, _ value: {swift_ty}) -> Result<Void, SDRKitCode> {{\n\
                     \x20       sdrkitResult({call})\n\
                     \x20   }}\n"
                );
            }
        }
        out.push_str("}\n");
    }
    for n in &names {
        if let Some(word) = doubled_word(n) {
            return Err(format!("the name {n} has the word {word} twice in a row"));
        }
    }
    Ok(out)
}
