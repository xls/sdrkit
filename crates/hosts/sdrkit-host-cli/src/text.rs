//! Text in and out: numbers with units and values of a declared kind from the command line, and the one formatter of a
//! value by its declaration for `show`, `run` and `events`.

use crate::error::CliError;
use sdrkit_core::base64::{decode_into, Alphabet};
use sdrkit_core::{FieldDecl, Value, ValueKind};
use std::fmt::Write as _;
use std::sync::Arc;

/// The unit of a number that has no unit suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bare {
    /// A frequency: a number below 100 000 is MHz, a larger one is Hz.
    Frequency,
    /// A rate or a bandwidth: Hz.
    Hz,
}

/// Reads a number of hertz: `919.875M`, `919.875MHz`, `919875000`, `2.4M`, `48k`, `1.2G`. A number without a unit is read
/// as `bare` says.
///
/// # Errors
///
/// [`CliError::BadNumber`] with the accepted forms.
pub fn parse_hz(text: &str, bare: Bare) -> Result<f64, CliError> {
    let bad = || CliError::BadNumber {
        text: text.to_owned(),
    };
    let t = text.trim();
    let t = t
        .strip_suffix("Hz")
        .or_else(|| t.strip_suffix("hz"))
        .or_else(|| t.strip_suffix("HZ"))
        .unwrap_or(t);
    let (num, scale) = match t.as_bytes().last() {
        Some(b'k' | b'K') => (&t[..t.len() - 1], Some(1e3)),
        Some(b'M') => (&t[..t.len() - 1], Some(1e6)),
        Some(b'G' | b'g') => (&t[..t.len() - 1], Some(1e9)),
        _ => (t, None),
    };
    let v: f64 = num.trim().parse().map_err(|_| bad())?;
    if !v.is_finite() {
        return Err(bad());
    }
    Ok(match (scale, bare) {
        (Some(s), _) => v * s,
        (None, Bare::Frequency) if v.abs() < 1e5 => v * 1e6,
        (None, _) => v,
    })
}

/// Splits `key=value`.
///
/// # Errors
///
/// [`CliError::NotKeyValue`] if there is no `=` or the key is empty.
pub fn key_value(text: &str) -> Result<(&str, &str), CliError> {
    match text.split_once('=') {
        Some((k, v)) if !k.trim().is_empty() => Ok((k.trim(), v.trim())),
        _ => Err(CliError::NotKeyValue {
            text: text.to_owned(),
        }),
    }
}

/// Reads `text` as a value of the field `decl`: yes or no, a number, a frequency with a unit, a length of time in
/// seconds, `lat,lon`, a choice by its key, label or number, or base64 bytes.
///
/// # Errors
///
/// [`CliError::BadValue`] with the forms or the choices that the field takes.
pub fn parse_value(decl: &FieldDecl, text: &str) -> Result<Value, CliError> {
    let bad = |forms: String| CliError::BadValue {
        key: decl.key.to_string(),
        text: text.to_owned(),
        kind: decl.kind,
        forms,
    };
    let number = || text.trim().parse::<f64>().ok().filter(|v| v.is_finite());
    let v = match decl.kind {
        ValueKind::Flags => return Err(bad(": a flag set is a result, not an input".into())),
        ValueKind::Bool => match text.to_ascii_lowercase().as_str() {
            "yes" | "true" | "on" | "1" => Value::Bool(true),
            "no" | "false" | "off" | "0" => Value::Bool(false),
            _ => return Err(bad(": yes or no".into())),
        },
        ValueKind::I64 => Value::I64(
            text.trim()
                .parse()
                .map_err(|_| bad(": a whole number".into()))?,
        ),
        ValueKind::F64 => Value::F64(number().ok_or_else(|| bad(": a number".into()))?),
        ValueKind::Text => Value::Text(text.to_owned()),
        ValueKind::Hz => {
            Value::Hz(parse_hz(text, Bare::Hz).map_err(|_| bad(": 919.875M, 48k, 250000".into()))?)
        }
        ValueKind::Time => Value::Time(number().ok_or_else(|| bad(": seconds since 1970".into()))?),
        ValueKind::Duration => {
            Value::Duration(parse_duration(text).ok_or_else(|| bad(": 30, 30s, 5min, 2h".into()))?)
        }
        ValueKind::Position => {
            let pos = text.split_once(',').and_then(|(a, b)| {
                Some((a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?))
            });
            let (lat, lon) = pos.ok_or_else(|| bad(": LAT,LON in degrees".into()))?;
            Value::Position { lat, lon }
        }
        ValueKind::Choice => {
            let i = decl
                .choices
                .iter()
                .position(|c| {
                    c.key.eq_ignore_ascii_case(text) || c.label.eq_ignore_ascii_case(text)
                })
                .or_else(|| {
                    text.parse::<usize>()
                        .ok()
                        .filter(|i| *i < decl.choices.len())
                });
            match i.and_then(|i| u16::try_from(i).ok()) {
                Some(i) => Value::Choice(i),
                None => {
                    let keys: Vec<&str> = decl.choices.iter().map(|c| c.key.as_ref()).collect();
                    return Err(bad(format!(": one of {}", keys.join(", "))));
                }
            }
        }
        ValueKind::Bytes => {
            let mut out = Vec::new();
            decode_into(text, Alphabet::default(), &mut out).map_err(|_| bad(": base64".into()))?;
            Value::Bytes(Arc::from(out))
        }
        ValueKind::Any => return Err(bad(": this field takes its kind from its row".into())),
    };
    Ok(v)
}

/// Reads a length of time: seconds, or a number with `s`, `min` or `h`.
fn parse_duration(text: &str) -> Option<f64> {
    let t = text.trim();
    let (num, scale) = if let Some(n) = t.strip_suffix("min") {
        (n, 60.0)
    } else if let Some(n) = t.strip_suffix('h') {
        (n, 3600.0)
    } else if let Some(n) = t.strip_suffix('s') {
        (n, 1.0)
    } else {
        (t, 1.0)
    };
    num.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v * scale)
}

/// Writes `v` without trailing zeros, with at most `decimals` decimals.
fn plain(out: &mut String, v: f64, decimals: usize) {
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.')
    } else {
        &s
    };
    out.push_str(if s == "-0" { "0" } else { s });
}

/// A frequency with the prefix that fits: `919.875 MHz`, `250 kHz`.
pub fn hz(v: f64) -> String {
    let mut s = String::new();
    let (scale, unit, decimals) = match v.abs() {
        a if a >= 1e9 => (1e9, "GHz", 6),
        a if a >= 1e6 => (1e6, "MHz", 6),
        a if a >= 1e3 => (1e3, "kHz", 3),
        _ => (1.0, "Hz", 1),
    };
    plain(&mut s, v / scale, decimals);
    s.push(' ');
    s.push_str(unit);
    s
}

/// A length of time: `0.25 s`, `54 s`, `12 min`, `3.5 h`, `2 d`.
pub fn duration(s: f64) -> String {
    let mut out = String::new();
    let a = s.abs();
    let (v, unit, decimals) = match a {
        a if a < 10.0 => (s, "s", 2),
        a if a < 120.0 => (s, "s", 0),
        a if a < 7200.0 => (s / 60.0, "min", 0),
        a if a < 172_800.0 => (s / 3600.0, "h", 1),
        _ => (s / 86_400.0, "d", 0),
    };
    plain(&mut out, v, decimals);
    out.push(' ');
    out.push_str(unit);
    out
}

/// An age of a time, from `now` (both seconds since 1970): `now`, `54 s ago`, `in 2 min`.
pub fn age(t: f64, now: f64) -> String {
    let d = now - t;
    if d.abs() < 1.0 {
        return "now".into();
    }
    if d > 0.0 {
        format!("{} ago", duration(d))
    } else {
        format!("in {}", duration(-d))
    }
}

/// Formats a value by the kind and the unit of its declaration. `now` is the wall time, for the age of a time.
pub fn cell(decl: &FieldDecl, v: &Value, now: f64) -> String {
    let unit = decl.unit.as_ref();
    let mut s = String::new();
    match v {
        Value::None => {}
        Value::Hz(x) => s = hz(*x),
        Value::F64(x) if unit == "Hz" => s = hz(*x),
        Value::Duration(x) => s = duration(*x),
        Value::F64(x) if unit == "s" => s = duration(*x),
        Value::Time(t) => s = age(*t, now),
        Value::F64(x) if unit.starts_with("dB") || unit == "%" => s = format!("{x:.1} {unit}"),
        Value::Position { lat, lon } => s = format!("{lat:.4}, {lon:.4}"),
        Value::F64(_) | Value::I64(_) if !unit.is_empty() => {
            let _ = write!(s, "{} {unit}", v.display(&decl.choices).with_flags(&decl.flags));
        }
        _ => {
            let _ = write!(s, "{}", v.display(&decl.choices).with_flags(&decl.flags));
        }
    }
    s
}

/// Writes rows as a table with aligned columns. A cell longer than 60 characters is cut.
pub fn table(headings: &[String], rows: &[Vec<String>], out: &mut String) {
    const MAX: usize = 60;
    let cut = |s: &str| -> String {
        if s.chars().count() > MAX {
            let mut c: String = s.chars().take(MAX - 3).collect();
            c.push_str("...");
            c
        } else {
            s.to_owned()
        }
    };
    let mut width: Vec<usize> = headings.iter().map(|h| h.chars().count()).collect();
    for r in rows {
        for (w, c) in width.iter_mut().zip(r) {
            *w = (*w).max(cut(c).chars().count());
        }
    }
    let line = |cells: &[String], out: &mut String| {
        let mut l = String::new();
        for (i, (c, w)) in cells.iter().zip(&width).enumerate() {
            let c = cut(c);
            if i + 1 == cells.len() {
                l.push_str(&c);
            } else {
                let _ = write!(l, "{c:<w$}  ");
            }
        }
        out.push_str(l.trim_end());
        out.push('\n');
    };
    line(headings, out);
    for r in rows {
        line(r, out);
    }
}
