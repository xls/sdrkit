//! The page of one stack, written from its descriptor and the declarations of a receiver made with the default settings.

use crate::render::{
    code, commands_table, events_section, hz, kind_name, mhz, results_sections, settings_table,
    table,
};
use crate::source::{self, Home};
use sdrkit_core::interface::{Family, Shape};
use sdrkit_core::{
    AdapterCatalog, Choice, Facilities, Receiver, StackConfig, StackDescriptor, Values,
};
use std::collections::BTreeMap;
use std::sync::Arc;

/// What a page needs from the engine side: the host's adapters and facilities.
pub struct Ctx {
    pub adapters: Arc<dyn AdapterCatalog>,
    pub facilities: Facilities,
    pub homes: BTreeMap<String, Home>,
}

impl Ctx {
    pub fn new() -> Ctx {
        let facilities = Facilities::default();
        let registry = sdrkit_host_catalog::registry(&facilities);
        Ctx {
            adapters: registry.catalog().clone(),
            facilities,
            homes: source::homes(),
        }
    }

    /// Makes sure that the receiver of stack `d` can be made with its default settings, which a page needs.
    ///
    /// # Errors
    ///
    /// A sentence with the reason.
    pub fn starts(&self, d: &StackDescriptor) -> Result<(), String> {
        let cfg = StackConfig::new(Values::defaults(&(d.settings)()))
            .with_adapters(Arc::clone(&self.adapters));
        (d.make_receiver)(&cfg)
            .map(|_| ())
            .map_err(|e| format!("the stack {} does not start with its defaults: {e}", d.id))
    }

    fn receiver(&self, d: &StackDescriptor) -> Box<dyn Receiver> {
        let cfg = StackConfig::new(Values::defaults(&(d.settings)()))
            .with_adapters(Arc::clone(&self.adapters));
        let mut rx = (d.make_receiver)(&cfg)
            .unwrap_or_else(|e| panic!("the stack {} does not start with its defaults: {e}", d.id));
        rx.set_facilities(&self.facilities);
        rx
    }
}

fn tuning_rows(d: &StackDescriptor) -> Vec<Vec<String>> {
    let t = &d.tuning;
    let mut rows = vec![vec!["Default frequency".to_string(), hz(t.default_hz)]];
    rows.push(vec![
        "Tuning range".into(),
        if t.min_hz <= 0.0 && t.max_hz >= 1e12 {
            "Anywhere the radio tunes".into()
        } else {
            format!("{} to {}", hz(t.min_hz), hz(t.max_hz))
        },
    ]);
    rows.push(vec!["Sample rate".into(), hz(t.rate_hz)]);
    rows.push(vec!["Smallest sample rate".into(), hz(t.min_rate_hz)]);
    rows.push(vec!["Signal bandwidth".into(), hz(t.bandwidth_hz)]);
    if t.tune_offset_hz != 0.0 {
        rows.push(vec!["Offset from the signal".into(), hz(t.tune_offset_hz)]);
    }
    if let Some(f) = t.radio_filter_hz {
        rows.push(vec!["Radio filter".into(), hz(f)]);
    }
    rows.push(vec!["Automatic gain".into(), d.auto_gain.label().into()]);
    rows
}

/// True if the settings of the stack have a region.
fn has_region(d: &StackDescriptor) -> bool {
    (d.settings)()
        .settings()
        .iter()
        .any(|s| s.field.key.ends_with("region") && s.field.label.to_lowercase().contains("region"))
}

fn try_it(d: &StackDescriptor) -> String {
    let mut out = String::from("## Try it\n\n```sh\n");
    if d.make_generator.is_some() {
        out.push_str(&format!("sdrkit run {0} --source synth:{0}\n", d.id));
        if let Some(s) = d.submodes.first() {
            out.push_str(&format!(
                "sdrkit run {0} --source synth:{0} --submode {1}\n",
                d.id, s.id
            ));
        }
    }
    let t = &d.tuning;
    if has_region(d) {
        out.push_str(&format!(
            "sdrkit run {} --source iiod:192.168.2.1 --region AU\n",
            d.id
        ));
    } else if (70e6..=6e9).contains(&t.default_hz) {
        out.push_str(&format!(
            "sdrkit run {} --source iiod:192.168.2.1 --freq {}\n",
            d.id,
            mhz(t.default_hz)
        ));
    } else {
        out.push_str(&format!(
            "sdrkit run {} --source <driver>:<address> --freq {}\n",
            d.id,
            mhz(t.default_hz)
        ));
    }
    out.push_str("```\n\n");
    if d.make_generator.is_none() {
        out.push_str("The stack has no test signal. The `synth` source plays silence for it.\n\n");
    }
    if has_region(d) {
        out.push_str("The region sets the channel plan. `sdrkit station` shows the region of the station.\n\n");
    }
    out
}

/// The page of stack `d`.
pub fn page(d: &StackDescriptor, ctx: &Ctx) -> String {
    let rx = ctx.receiver(d);
    let mut out = format!("# {}\n\n{}\n\n", d.name, d.blurb);
    out.push_str(&format!(
        "Stack id: {}. Group: {}.\n\n",
        code(d.id),
        d.group
    ));
    let home = ctx.homes.get(d.id);
    if let Some(h) = home {
        let (text, standard) = source::describe(h);
        if !text.is_empty() {
            out.push_str(&format!("{text}\n\n"));
        }
        if !standard.is_empty() {
            out.push_str(&format!("Standard: {standard}\n\n"));
        }
    }
    if !d.signal_types.is_empty() {
        let names: Vec<&str> = d.signal_types.iter().map(|t| t.label()).collect();
        out.push_str(&format!("Signal types: {}.\n\n", names.join(", ")));
    }

    out.push_str("## Tuning\n\n");
    out.push_str(&table(&["Property", "Value"], &tuning_rows(d)));
    out.push('\n');

    if !d.submodes.is_empty() {
        out.push_str("## Submodes\n\nChoose one with `--submode <id>`.\n\n");
        let rows: Vec<Vec<String>> = d
            .submodes
            .iter()
            .map(|s| vec![code(s.id), s.name.to_string(), hz(s.default_hz)])
            .collect();
        out.push_str(&table(&["Id", "Name", "Default frequency"], &rows));
        out.push('\n');
    }

    out.push_str(&settings_section(d));

    let decl = rx.results_decl();
    out.push_str(&results_sections(decl, "Result fields"));
    if !decl.events().is_empty() {
        out.push_str(&events_section(decl.events()));
    }
    let commands = rx.commands();
    if !commands.is_empty() {
        out.push_str("## Commands\n\n");
        out.push_str(&commands_table(commands.defs()));
        out.push('\n');
    }

    if let Some(f) = sdrkit_host_catalog::family_of(d.id) {
        out.push_str(&interface_section(d, f));
    }

    out.push_str(&try_it(d));

    if let Some(h) = home {
        let mut names: Vec<&str> = vec![d.id];
        names.extend(d.submodes.iter().map(|s| s.id));
        let found = source::vectors(&h.family, &names);
        if !found.is_empty() {
            out.push_str("## Test recordings\n\n");
            for f in found {
                out.push_str(&format!("- {}\n", code(&f)));
            }
            out.push('\n');
        }
    }
    out
}

/// The settings section: a table of the schema, or a line that says there are none.
pub fn settings_section(d: &StackDescriptor) -> String {
    let schema = (d.settings)();
    let mut out = String::from("## Settings\n\n");
    if schema.is_empty() {
        out.push_str("The stack has no settings.\n\n");
        return out;
    }
    out.push_str(&format!(
        "Give a value as `key=value` after the stack name. `sdrkit settings kind:stack:{}` prints this table with every option.\n\n",
        d.id
    ));
    out.push_str(&settings_table(schema.settings()));
    out.push('\n');
    out
}

/// The fields of `shape` as a table: key, kind, whether a value is always there, the version, and what the field holds.
fn shape_table(family: &Family, title: &str, shape: &Shape) -> String {
    let rows: Vec<Vec<String>> = shape
        .fields
        .iter()
        .zip(shape.notes)
        .map(|(f, note)| {
            let decl = f.decl();
            let since = family.since(&format!("shape.{}.{}", shape.name, decl.key));
            vec![
                code(&decl.key),
                kind_name(&decl),
                if note.optional { "optional" } else { "always" }.to_string(),
                since.to_string(),
                note.doc.to_string(),
            ]
        })
        .collect();
    format!(
        "### {title}\n\nStruct {}. {}\n\n{}\n",
        code(shape.name),
        shape.doc,
        table(&["Key", "Kind", "Value", "Since", "Description"], &rows)
    )
}

/// The typed interface of stack `d` in `family`: the version, and the struct of each result set, table and event type.
fn interface_section(d: &StackDescriptor, family: &Family) -> String {
    let Some(stack) = family.stack_shape(d.id) else {
        return String::new();
    };
    let mut out = format!(
        "## Typed interface\n\nThe stack is in the family {}, version {}. The C header {}, the registry record {} and the tables below are written from one description. A struct holds every value of a row, a result set or an event. A command and a setting have a function each.\n\n",
        code(family.key),
        family.version,
        code(&format!("sdrkit/{}.h", family.key)),
        code(&format!("registry/{}.reg", family.key)),
    );
    if let Some(line) = sdrkit_dev_interface::marks::stack_sentence(stack) {
        out.push_str(&format!("{line}\n\n"));
    }
    for f in &stack.fields {
        out.push_str(&shape_table(family, "Result fields", f));
    }
    for t in &stack.tables {
        let title = format!("Table {}", code(t.def.key));
        out.push_str(&shape_table(family, &title, &t.row));
        let marks = sdrkit_dev_interface::marks::table_sentences(t.def);
        if !marks.is_empty() {
            out.push_str(&format!("{}\n\n", marks.join(" ")));
        }
    }
    for e in &stack.events {
        let title = format!("Event {}", code(e.def.kind));
        out.push_str(&shape_table(family, &title, &e.fields));
    }
    use sdrkit_dev_interface::c_header::{command_function, setting_functions, struct_name};
    if !stack.commands.is_empty() {
        let rows: Vec<Vec<String>> = stack
            .commands
            .iter()
            .map(|c| {
                let name = |n: &str| {
                    if n.is_empty() {
                        "none".to_string()
                    } else {
                        code(&struct_name(family, n))
                    }
                };
                vec![
                    code(c.command.key),
                    code(&command_function(d.id, c.command.key)),
                    name(c.args.name),
                    name(c.reply.name),
                ]
            })
            .collect();
        out.push_str("### Commands\n\n");
        out.push_str(&table(
            &["Key", "C function", "Arguments struct", "Reply struct"],
            &rows,
        ));
        out.push('\n');
    }
    // The settings of the receiver, then those of the test signal generator.
    for (title, generator) in [
        ("Settings", false),
        ("Settings of the test signal generator", true),
    ] {
        let rows: Vec<Vec<String>> = stack
            .settings
            .iter()
            .map(|s| s.key())
            .filter(|key| sdrkit_dev_interface::generator_setting(key).is_some() == generator)
            .map(|key| {
                let (get, set) = setting_functions(d.id, key);
                vec![code(key), code(&get), code(&set)]
            })
            .collect();
        if !rows.is_empty() {
            out.push_str(&format!("### {title}\n\n"));
            out.push_str(&table(&["Key", "C getter", "C setter"], &rows));
            out.push('\n');
        }
    }
    out
}
