//! Values as text for the screen, and text typed by the user as values of a declared type.

use crate::api::{Decl, Type, Value};
use std::fmt::{self, Write};

/// Why typed text is not a value of its declared type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The text is not a number.
    NotANumber,
    /// The text is not yes or no.
    NotYesNo,
    /// The text is not two numbers, latitude and longitude.
    NotAPosition,
    /// The text is not the key, the label or the index of an option.
    NotAnOption,
    /// The text is not bytes in hexadecimal.
    NotHex,
    /// The declaration has no type that takes a value.
    NoValue,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ParseError::NotANumber => "not a number",
            ParseError::NotYesNo => "type yes or no",
            ParseError::NotAPosition => "type latitude,longitude in degrees",
            ParseError::NotAnOption => "not one of the options",
            ParseError::NotHex => "type the bytes in hexadecimal",
            ParseError::NoValue => "this value cannot be typed",
        })
    }
}

impl std::error::Error for ParseError {}

/// Writes `x` with at most `places` decimals and no trailing zeros.
fn number(out: &mut String, x: f64, places: usize) {
    let start = out.len();
    let _ = write!(out, "{x:.places$}");
    if out[start..].contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
}

/// Writes the frequency `hz` with the unit that suits it: Hz, kHz, MHz or GHz.
pub fn hz(out: &mut String, hz: f64) {
    let a = hz.abs();
    let (scale, unit, places) = if a >= 1e9 {
        (1e9, "GHz", 6)
    } else if a >= 1e6 {
        (1e6, "MHz", 4)
    } else if a >= 1e3 {
        (1e3, "kHz", 3)
    } else {
        (1.0, "Hz", 1)
    };
    number(out, hz / scale, places);
    out.push(' ');
    out.push_str(unit);
}

/// The offset of local time from UTC that a time is shown with, minutes east: the engine setting `time_zone`.
static ZONE_MIN: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);

/// Sets the offset of local time from UTC that every time is shown with, minutes east.
pub fn set_zone(minutes: i64) {
    ZONE_MIN.store(minutes, std::sync::atomic::Ordering::Relaxed);
}

/// The offset of the local time of this computer from UTC now, minutes east. 0 if the system does not say.
pub fn system_zone() -> i64 {
    #[cfg(unix)]
    {
        // SAFETY: `time` and `localtime_r` write only into the two values given to them.
        unsafe {
            let now = libc::time(std::ptr::null_mut());
            let mut tm: libc::tm = std::mem::zeroed();
            if libc::localtime_r(&now, &mut tm).is_null() {
                return 0;
            }
            i64::from(tm.tm_gmtoff as i32) / 60
        }
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Writes `secs` seconds since 1970 as a date and time in the zone of [`set_zone`], `YYYY-MM-DD HH:MM:SS`.
fn utc(out: &mut String, secs: f64) {
    if !secs.is_finite() {
        out.push('-');
        return;
    }
    let secs = secs + 60.0 * ZONE_MIN.load(std::sync::atomic::Ordering::Relaxed) as f64;
    // A time in range of a date fits in i64 seconds; the cast saturates outside it.
    let s = secs.floor() as i64;
    let (days, rem) = (s.div_euclid(86_400), s.rem_euclid(86_400));
    // Days to a civil date in the proleptic Gregorian calendar (ISO 8601).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let _ = write!(
        out,
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    );
}

/// Returns true if `v` is the special number of `decl` and `decl` declares what it means.
fn meant_zero(v: &Value, decl: &Decl) -> bool {
    let zero = match v {
        Value::I64(i) => *i as f64 == decl.special_value,
        Value::F64(x) | Value::Hz(x) | Value::Time(x) | Value::Duration(x) => *x == decl.special_value,
        _ => false,
    };
    zero && !decl.zero_means.is_empty()
}

/// Writes the number `x` as a value of the type of `decl`, with its unit.
pub fn quantity(out: &mut String, x: f64, decl: &Decl) {
    match decl.ty {
        Type::Hz => hz(out, x),
        Type::Duration => {
            number(out, x, 3);
            out.push_str(" s");
        }
        _ => {
            number(out, x, 3);
            if !decl.unit.is_empty() {
                out.push(' ');
                out.push_str(&decl.unit);
            }
        }
    }
}

/// Writes `v` as text for the screen, with the unit and the option labels of `decl`. A 0 that `decl` declares a meaning
/// for is written as the meaning.
pub fn value(out: &mut String, v: &Value, decl: &Decl) {
    if meant_zero(v, decl) {
        out.push_str(&decl.zero_means);
        return;
    }
    let unit = |out: &mut String| {
        if !decl.unit.is_empty() {
            out.push(' ');
            out.push_str(&decl.unit);
        }
    };
    match v {
        Value::None => {}
        Value::Bool(b) => out.push_str(if *b { "yes" } else { "no" }),
        Value::I64(i) => {
            let _ = write!(out, "{i}");
            unit(out);
        }
        Value::F64(x) => {
            if x.is_finite() {
                number(out, *x, 3);
                unit(out);
            } else {
                out.push('-');
            }
        }
        Value::String(s) => out.push_str(s),
        Value::Hz(x) => hz(out, *x),
        Value::Time(t) => utc(out, *t),
        Value::Duration(d) => {
            number(out, *d, 3);
            out.push_str(" s");
        }
        Value::Position(lat, lon) => {
            number(out, *lat, 5);
            out.push_str(", ");
            number(out, *lon, 5);
        }
        Value::Enum(i) => match decl.options.get(*i as usize) {
            Some(o) => out.push_str(&o.label),
            None => {
                let _ = write!(out, "#{i}");
            }
        },
        Value::Bytes(b) => {
            let _ = write!(out, "{} bytes", b.len());
            if !b.is_empty() {
                out.push(' ');
                for x in b.iter().take(8) {
                    let _ = write!(out, "{x:02x}");
                }
                if b.len() > 8 {
                    out.push('…');
                }
            }
        }
    }
}

/// Returns `v` as text for the screen.
pub fn value_text(v: &Value, decl: &Decl) -> String {
    let mut s = String::new();
    value(&mut s, v, decl);
    s
}

/// Returns true if the editor of a value of `decl` starts with an empty line: a number is typed anew, a text is
/// changed.
pub fn typed_anew(decl: &Decl) -> bool {
    matches!(
        decl.ty,
        Type::I64 | Type::F64 | Type::Hz | Type::Time | Type::Duration | Type::Position
    )
}

/// Returns `v` as text that [`parse`] reads back to the same value. A 0 that `decl` declares a meaning for is the empty
/// text.
pub fn edit_text(v: &Value, decl: &Decl) -> String {
    let mut s = String::new();
    if meant_zero(v, decl) {
        return s;
    }
    match v {
        Value::Hz(x) => number(&mut s, *x, 3),
        Value::F64(x) | Value::Time(x) | Value::Duration(x) => number(&mut s, *x, 9),
        Value::Enum(i) => {
            if let Some(o) = decl.options.get(*i as usize) {
                s.push_str(&o.key);
            }
        }
        Value::Position(lat, lon) => {
            let _ = write!(s, "{lat},{lon}");
        }
        Value::Bytes(b) => {
            for x in b {
                let _ = write!(s, "{x:02x}");
            }
        }
        _ => value(&mut s, v, decl),
    }
    s
}

/// Reads a number with an optional suffix k, M or G in either case and an optional unit `Hz`: `105.6M`, `0105.6 MHz`,
/// `919875k`. Returns the number and the factor of the suffix, or `None` for a number without a suffix.
fn split_hz(text: &str) -> Result<(f64, Option<f64>), ParseError> {
    let t = text.trim();
    let t = match t.len().checked_sub(2).and_then(|n| t.split_at_checked(n)) {
        Some((head, unit)) if unit.eq_ignore_ascii_case("hz") => head.trim_end(),
        _ => t,
    };
    let (digits, scale) = match t.chars().last() {
        Some('k' | 'K') => (&t[..t.len() - 1], Some(1e3)),
        Some('m' | 'M') => (&t[..t.len() - 1], Some(1e6)),
        Some('g' | 'G') => (&t[..t.len() - 1], Some(1e9)),
        _ => (t, None),
    };
    let x = digits
        .trim()
        .parse::<f64>()
        .map_err(|_| ParseError::NotANumber)?;
    if x.is_finite() {
        Ok((x, scale))
    } else {
        Err(ParseError::NotANumber)
    }
}

/// Reads `text` as a frequency to tune to, in Hz. A number without a suffix is in MHz: `105.6`, `105.6M` and
/// `0105.6 MHz` are the same frequency.
///
/// # Errors
///
/// [`ParseError::NotANumber`] if the text is not a number.
pub fn parse_frequency(text: &str) -> Result<f64, ParseError> {
    let (x, scale) = split_hz(text)?;
    Ok(x * scale.unwrap_or(1e6))
}

/// Reads `text` as a value in Hz of `decl`. A number without a suffix is in Hz; if that is outside the range of `decl`
/// and the same number in MHz is inside, it is in MHz.
fn parse_hz(text: &str, decl: &Decl) -> Result<f64, ParseError> {
    let (x, scale) = split_hz(text)?;
    if let Some(scale) = scale {
        return Ok(x * scale);
    }
    let inside = |v: f64| decl.range.is_some_and(|(min, max, _)| v >= min && v <= max);
    let meant = x == decl.special_value && !decl.zero_means.is_empty();
    if !meant && !inside(x) && inside(x * 1e6) {
        Ok(x * 1e6)
    } else {
        Ok(x)
    }
}

/// Reads `text` as a value of the type of `decl`. The library checks the range and the options after.
///
/// A frequency takes a suffix k, M or G (`104.9M`); a number without a suffix is in Hz, or in MHz where only that is
/// inside the range. An option is its key, its label or its index. Bytes are hexadecimal. A position is
/// `latitude,longitude`.
///
/// # Errors
///
/// [`ParseError`] if the text is not of the type.
/// `text` without the unit `unit` at its end, in any letter case. `None` if `unit` is empty or `text` does not end in
/// it.
fn strip_unit<'a>(text: &'a str, unit: &str) -> Option<&'a str> {
    let at = text.len().checked_sub(unit.len())?;
    (!unit.is_empty() && text.is_char_boundary(at) && text[at..].eq_ignore_ascii_case(unit))
        .then(|| text[..at].trim_end())
}

pub fn parse(text: &str, decl: &Decl) -> Result<Value, ParseError> {
    let t = text.trim();
    // A number may end in the unit of the setting: "3 dB" is 3.
    let t = match (decl.ty, strip_unit(t, &decl.unit)) {
        (Type::I64 | Type::F64 | Type::Time | Type::Duration, Some(n)) => n,
        _ => t,
    };
    let num = || t.parse::<f64>().map_err(|_| ParseError::NotANumber);
    Ok(match decl.ty {
        Type::None => return Err(ParseError::NoValue),
        Type::Bool => match t.to_ascii_lowercase().as_str() {
            "yes" | "y" | "on" | "true" | "1" => Value::Bool(true),
            "no" | "n" | "off" | "false" | "0" => Value::Bool(false),
            _ => return Err(ParseError::NotYesNo),
        },
        Type::I64 => Value::I64(t.parse().map_err(|_| ParseError::NotANumber)?),
        Type::F64 => Value::F64(num()?),
        Type::Hz => Value::Hz(parse_hz(t, decl)?),
        Type::Time => Value::Time(num()?),
        Type::Duration => Value::Duration(num()?),
        Type::String => Value::String(text.to_owned()),
        Type::Any => match t.parse::<f64>() {
            Ok(x) => Value::F64(x),
            Err(_) => Value::String(text.to_owned()),
        },
        Type::Position => {
            let (a, b) = t.split_once(',').ok_or(ParseError::NotAPosition)?;
            let lat = a.trim().parse().map_err(|_| ParseError::NotAPosition)?;
            let lon = b.trim().parse().map_err(|_| ParseError::NotAPosition)?;
            Value::Position(lat, lon)
        }
        Type::Enum => {
            let by_name = decl
                .options
                .iter()
                .position(|o| o.key == t || o.label.eq_ignore_ascii_case(t));
            match by_name {
                Some(i) => Value::Enum(i as u32),
                None => Value::Enum(t.parse().map_err(|_| ParseError::NotAnOption)?),
            }
        }
        Type::Bytes => {
            let hex: String = t.chars().filter(|c| !c.is_whitespace()).collect();
            if !hex.is_ascii() || !hex.len().is_multiple_of(2) {
                return Err(ParseError::NotHex);
            }
            let bytes: Result<Vec<u8>, _> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
                .collect();
            Value::Bytes(bytes.map_err(|_| ParseError::NotHex)?)
        }
    })
}
