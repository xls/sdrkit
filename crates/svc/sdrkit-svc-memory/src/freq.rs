//! Frequencies typed by people or found in lists: `"145.5"`, `"145.500 MHz"`, `"7100 kHz"`, `"7.1M"`, `"433920000"`, `"1,090 MHz"`.

/// The unit a number without its own unit is read in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnitHint {
    /// Guess from the size of the number (see [`parse`]).
    #[default]
    Guess,
    /// Hertz.
    Hz,
    /// Kilohertz.
    KHz,
    /// Megahertz.
    MHz,
    /// Gigahertz.
    GHz,
}

impl UnitHint {
    /// The unit a column header names (`"Frequency (kHz)"`, `"MHz"`), or [`UnitHint::Guess`].
    pub fn from_header(header: &str) -> UnitHint {
        let h = header.to_ascii_lowercase();
        if h.contains("ghz") {
            UnitHint::GHz
        } else if h.contains("mhz") {
            UnitHint::MHz
        } else if h.contains("khz") {
            UnitHint::KHz
        } else if h.contains("hz") {
            UnitHint::Hz
        } else {
            UnitHint::Guess
        }
    }
}

/// A parsed frequency and how finely it was typed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parsed {
    /// The frequency, Hz.
    pub hz: f64,
    /// One step of the last digit typed, Hz (`"145.5"` MHz has a resolution of 100 kHz). A search matches within half of it.
    pub resolution_hz: f64,
}

/// Parses a frequency. A unit in the text (`Hz`, `k`, `kHz`, `M`, `MHz`, `G`, `GHz`, any case) wins over `hint`. Without either, the
/// size decides: below 30 000 it is MHz (`"145.5"`, `"1090"`), from 30 000 to below 3 000 000 kHz (`"100000"` is 100 MHz), and from
/// 3 000 000 up Hz. A short-wave list in kHz (`"7100"`) must therefore say so in its header or its text. Thousands separators (`,` `_` `'` and spaces between
/// digit groups) are removed when the number also has a decimal point or more than one separator; a single comma with no point is a
/// decimal comma (`"145,5"`).
pub fn parse(text: &str, hint: UnitHint) -> Option<Parsed> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    let split = t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len());
    let (num, unit) = t.split_at(split);
    let unit = unit.trim().to_ascii_lowercase();
    let unit = match unit.as_str() {
        "" => hint,
        "hz" | "h" => UnitHint::Hz,
        "k" | "khz" => UnitHint::KHz,
        "m" | "mhz" => UnitHint::MHz,
        "g" | "ghz" => UnitHint::GHz,
        _ => return None,
    };
    let mut digits: String = num
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_' && *c != '\'')
        .collect();
    let commas = digits.matches(',').count();
    if commas == 1 && !digits.contains('.') {
        digits = digits.replace(',', ".");
    } else {
        digits = digits.replace(',', "");
    }
    if digits.is_empty()
        || digits.starts_with('-')
        || !digits.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return None;
    }
    let value: f64 = digits.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let decimals = digits.find('.').map(|p| digits.len() - p - 1).unwrap_or(0) as i32;
    let scale = match unit {
        UnitHint::Hz => 1.0,
        UnitHint::KHz => 1e3,
        UnitHint::MHz => 1e6,
        UnitHint::GHz => 1e9,
        UnitHint::Guess if value < 30_000.0 => 1e6,
        UnitHint::Guess if value < 3_000_000.0 => 1e3,
        UnitHint::Guess => 1.0,
    };
    let hz = value * scale;
    if hz <= 0.0 {
        return None;
    }
    Some(Parsed {
        hz,
        resolution_hz: (scale * 10f64.powi(-decimals)).max(1.0),
    })
}

/// Parses a frequency and returns only its value in Hz.
pub fn parse_hz(text: &str, hint: UnitHint) -> Option<f64> {
    parse(text, hint).map(|p| p.hz)
}

/// Formats a frequency for people: `"145.500 MHz"`, `"7.100 MHz"`, `"198 kHz"`, `"10.489 GHz"`, with trailing zeros past three
/// decimals removed.
pub fn format(hz: f64) -> String {
    let (v, unit) = if hz >= 1e9 {
        (hz / 1e9, "GHz")
    } else if hz >= 1e6 {
        (hz / 1e6, "MHz")
    } else if hz >= 1e3 {
        (hz / 1e3, "kHz")
    } else {
        (hz, "Hz")
    };
    let mut s = format!("{v:.6}");
    while s.ends_with('0') && s.len() - s.find('.').unwrap_or(s.len()) > 4 {
        s.pop();
    }
    if unit == "Hz" || unit == "kHz" {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    format!("{s} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hz(t: &str) -> f64 {
        parse_hz(t, UnitHint::Guess).unwrap()
    }

    #[test]
    fn units_and_guesses() {
        assert_eq!(hz("145.5"), 145.5e6);
        assert_eq!(hz("145.500 MHz"), 145.5e6);
        assert_eq!(hz("7100 kHz"), 7.1e6);
        assert_eq!(hz("7100k"), 7.1e6);
        assert_eq!(hz("7.1M"), 7.1e6);
        assert_eq!(hz("1090"), 1090e6);
        assert_eq!(hz("433920000"), 433.92e6);
        assert_eq!(hz("10.489 GHz"), 10.489e9);
        assert_eq!(hz("1,090,000,000"), 1.09e9);
        assert_eq!(hz("145,5"), 145.5e6);
        assert_eq!(hz("100000"), 100e6);
        assert_eq!(parse_hz("7100", UnitHint::KHz), Some(7.1e6));
        assert_eq!(parse_hz("abc", UnitHint::Guess), None);
        assert_eq!(parse_hz("12 parsecs", UnitHint::Guess), None);
        assert_eq!(parse_hz("-5", UnitHint::Guess), None);
        assert_eq!(parse_hz("", UnitHint::Guess), None);
    }

    #[test]
    fn resolution_follows_the_digits_typed() {
        let p = parse("145.5", UnitHint::Guess).unwrap();
        assert!((p.resolution_hz - 100e3).abs() < 1e-6);
        let p = parse("145.525", UnitHint::Guess).unwrap();
        assert!((p.resolution_hz - 1e3).abs() < 1e-6);
        let p = parse("433920000", UnitHint::Guess).unwrap();
        assert_eq!(p.resolution_hz, 1.0);
    }

    #[test]
    fn header_hints() {
        assert_eq!(UnitHint::from_header("Frequency (kHz)"), UnitHint::KHz);
        assert_eq!(UnitHint::from_header("MHz"), UnitHint::MHz);
        assert_eq!(UnitHint::from_header("Freq"), UnitHint::Guess);
    }

    #[test]
    fn formats() {
        assert_eq!(format(145.5e6), "145.500 MHz");
        assert_eq!(format(145.5125e6), "145.5125 MHz");
        assert_eq!(format(198e3), "198 kHz");
        assert_eq!(format(10.489e9), "10.489 GHz");
    }
}
