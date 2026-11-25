//! The report of the keys that name a unit while their kind is a plain number: `centre_hz` declared as a number with
//! the unit text "Hz" in place of a frequency. A family corrects them in one pass, with a new major version if the
//! family is released.

use sdrkit_core::interface::{Family, StackShape};
use sdrkit_core::{FieldDecl, ValueKind};

/// The endings of a key that name a unit, each with the kind that holds the unit. `None`: no kind holds it yet.
const ENDINGS: [(&str, Option<ValueKind>); 10] = [
    ("_hz", Some(ValueKind::Hz)),
    ("_khz", Some(ValueKind::Hz)),
    ("_mhz", Some(ValueKind::Hz)),
    ("_s", Some(ValueKind::Duration)),
    ("_ms", Some(ValueKind::Duration)),
    ("_db", None),
    ("_dbm", None),
    ("_m", None),
    ("_km", None),
    ("_deg", None),
];

/// What the report says about the declaration `d`, if its key ends in a unit and its kind is a plain number.
fn finding(d: &FieldDecl) -> Option<String> {
    if !matches!(d.kind, ValueKind::F64 | ValueKind::I64) {
        return None;
    }
    let (ending, kind) = ENDINGS.iter().find(|(e, _)| d.key.ends_with(e))?;
    let kind_now = if d.kind == ValueKind::F64 {
        "number"
    } else {
        "whole number"
    };
    Some(match kind {
        Some(ValueKind::Hz) if *ending == "_hz" => {
            format!("{kind_now}, unit \"{}\": frequency", d.unit)
        }
        Some(ValueKind::Hz) => format!("{kind_now}, unit \"{}\": frequency, in Hz", d.unit),
        Some(_) if *ending == "_s" => format!("{kind_now}, unit \"{}\": length of time", d.unit),
        Some(_) => format!(
            "{kind_now}, unit \"{}\": length of time, in seconds",
            d.unit
        ),
        None => format!("{kind_now}, unit \"{}\": no kind for this unit", d.unit),
    })
}

fn of_stack(stack: &StackShape, out: &mut Vec<String>) {
    each_decl(stack, &mut |path: String, d: &FieldDecl| {
        if let Some(what) = finding(d) {
            let line = format!("{path} ({what})");
            if !out.contains(&line) {
                out.push(line);
            }
        }
    });
}

/// Calls `add` with the path and the declaration of each field, column, event field, setting, argument and reply
/// field of `stack`.
fn each_decl(stack: &StackShape, add: &mut dyn FnMut(String, &FieldDecl)) {
    let id = stack.id;
    for set in &stack.fields {
        for f in set.fields {
            add(format!("shape.{}.{}", set.name, f.key()), &f.decl());
        }
    }
    for set in stack.components.iter().filter_map(|c| c.shape) {
        for f in set.fields {
            add(format!("shape.{}.{}", set.name, f.key()), &f.decl());
        }
    }
    for t in &stack.tables {
        for f in t.row.fields {
            add(format!("shape.{}.{}", t.row.name, f.key()), &f.decl());
        }
    }
    for e in &stack.events {
        for f in e.fields.fields {
            add(format!("shape.{}.{}", e.fields.name, f.key()), &f.decl());
        }
    }
    for s in &stack.settings {
        let d = s.decl();
        add(format!("{id}.setting.{}", d.field.key), &d.field);
    }
    for c in &stack.commands {
        let def = c.command.def();
        for a in def.args.settings() {
            add(
                format!("{id}.command.{}.{}", c.command.key, a.field.key),
                &a.field,
            );
        }
        for f in c.reply.fields {
            add(format!("shape.{}.{}", c.reply.name, f.key()), &f.decl());
        }
    }
}

/// The spellings of a unit that are not used, each with the one spelling of that unit.
const SPELLINGS: [(&str, &str); 44] = [
    ("deg", "°"),
    ("degree", "°"),
    ("degrees", "°"),
    ("kn", "kt"),
    ("knot", "kt"),
    ("knots", "kt"),
    ("WPM", "wpm"),
    ("/s", "1/s"),
    ("/min", "1/min"),
    ("/h", "1/h"),
    ("byte", "B"),
    ("bytes", "B"),
    ("octet", "B"),
    ("octets", "B"),
    ("db", "dB"),
    ("DB", "dB"),
    ("dbm", "dBm"),
    ("dbfs", "dBFS"),
    ("dBfs", "dBFS"),
    ("hz", "Hz"),
    ("HZ", "Hz"),
    ("khz", "kHz"),
    ("KHz", "kHz"),
    ("mhz", "MHz"),
    ("Mhz", "MHz"),
    ("percent", "%"),
    ("pct", "%"),
    ("m/sec", "m/s"),
    ("mps", "m/s"),
    ("kmh", "km/h"),
    ("kph", "km/h"),
    ("sec", "s"),
    ("secs", "s"),
    ("second", "s"),
    ("seconds", "s"),
    ("msec", "ms"),
    ("usec", "us"),
    ("µs", "us"),
    ("degC", "°C"),
    ("C", "°C"),
    ("baud", "Bd"),
    ("bps", "bit/s"),
    ("b/s", "bit/s"),
    ("kbps", "kbit/s"),
];

/// The one spelling of the unit `unit`, if `unit` is another spelling of it.
pub fn spelling(unit: &str) -> Option<&'static str> {
    SPELLINGS
        .iter()
        .find(|(other, _)| *other == unit)
        .map(|(_, one)| *one)
}

/// One line for each unit of `family` that has another spelling than the one in use: of a field, a column, an event
/// field, a setting, an argument, a reply field, and of the axes of a series or a raster. A unit has one spelling in
/// every family: `°` for degrees, `kt` for knots, `B` for bytes, `1/s` for a rate. A shape is listed once.
pub fn spellings(family: &Family) -> Vec<String> {
    let mut out = Vec::new();
    let mut add = |path: String, unit: &str| {
        if let Some(one) = spelling(unit) {
            let line = format!(
                "{}: {path} has the unit \"{unit}\": write \"{one}\"",
                family.key
            );
            if !out.contains(&line) {
                out.push(line);
            }
        }
    };
    for stack in &family.stacks {
        each_decl(stack, &mut |path, d| add(path, &d.unit));
        for s in &stack.series {
            add(format!("{}.series.{} (x)", stack.id, s.key), s.x_unit);
            add(format!("{}.series.{} (y)", stack.id, s.key), s.y_unit);
        }
        for r in &stack.rasters {
            add(format!("{}.raster.{} (x)", stack.id, r.key), r.x_unit);
            add(format!("{}.raster.{} (y)", stack.id, r.key), r.y_unit);
            add(format!("{}.raster.{}", stack.id, r.key), r.unit);
        }
    }
    out
}

/// One line for each field, column, setting, argument and reply field of `family` whose key ends in `_hz`, `_khz`,
/// `_mhz`, `_s`, `_ms`, `_db`, `_dbm`, `_m`, `_km` or `_deg` and whose kind is a number or a whole number: the path,
/// the kind and the unit text that it has, and the kind that holds the unit. A shape is listed once.
pub fn report(family: &Family) -> Vec<String> {
    let mut out = Vec::new();
    for stack in &family.stacks {
        of_stack(stack, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::interface::{Fields, Hertz};

    /// A set with a frequency of each form.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct ToneFields {
        /// A frequency as a plain number.
        #[field(label = "Shift", unit = "Hz")]
        shift_hz: f64,
        /// A frequency with its kind.
        #[field(label = "Centre", unit = "Hz")]
        centre_hz: Hertz,
        /// A level.
        #[field(label = "Level", unit = "dB")]
        level_db: f64,
        /// A count.
        #[field(label = "Frames")]
        frames: i64,
    }

    #[test]
    fn a_key_with_a_unit_and_a_plain_number_is_listed_and_a_typed_one_is_not() {
        let family = Family::new("x", 1, 0).stack(StackShape::new("x").fields::<ToneFields>());
        let lines = report(&family);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("shape.ToneFields.shift_hz (number, unit \"Hz\": frequency"));
        assert!(lines[1].contains("level_db") && lines[1].contains("no kind"));
    }

    /// A set with a bearing in a spelling that is not used.
    #[derive(Fields)]
    #[allow(dead_code)]
    struct BearingFields {
        /// A bearing.
        #[field(label = "Bearing", unit = "deg")]
        bearing_deg: f64,
        /// A radial.
        #[field(label = "Radial", unit = "°")]
        radial_deg: f64,
    }

    #[test]
    fn a_unit_has_one_spelling() {
        let family = Family::new("x", 1, 0).stack(StackShape::new("x").fields::<BearingFields>());
        let lines = spellings(&family);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("bearing_deg") && lines[0].ends_with("write \"°\""));
        assert_eq!(spelling("knots"), Some("kt"));
        assert_eq!(spelling("°C"), None);
    }
}
