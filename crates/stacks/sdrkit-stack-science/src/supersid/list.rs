//! The list of monitored transmitters: the text of the `transmitters` setting, its parser, and the conversion of the rows of
//! a transmitter data file (CSV with the columns `label`, `frequency_hz`, `lat`, `lon`) into that text.
//!
//! Text form: one record per line or between `;`. A record is `label,frequency_hz` or `label,frequency_hz,lat,lon`
//! (degrees, north and east positive). Spaces around fields are ignored; empty records are skipped. A label holds no `,` or
//! `;`. At most [`MAX_TRANSMITTERS`] records.

use sdrkit_util_geo::Position;
use sdrkit_util_text::csv;

/// The most transmitters a list holds.
pub const MAX_TRANSMITTERS: usize = 32;

/// One monitored transmitter.
#[derive(Clone, Debug, PartialEq)]
pub struct Transmitter {
    /// The label (a call sign).
    pub label: String,
    /// The carrier frequency, Hz.
    pub frequency_hz: f64,
    /// The position, when the record gives one.
    pub position: Option<Position>,
}

/// Why a list text was refused: the byte offset of the record that does not parse, and the reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ListError {
    /// The byte offset of the record in the text.
    pub at: usize,
    /// What is wrong.
    pub reason: &'static str,
}

impl std::fmt::Display for ListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (at byte {})", self.reason, self.at)
    }
}

impl std::error::Error for ListError {}

/// Parses the text of the `transmitters` setting.
///
/// # Errors
///
/// [`ListError`] for a record without a label, with a frequency that is not a positive number, with one coordinate only,
/// with a coordinate out of range, and for more than [`MAX_TRANSMITTERS`] records.
pub fn parse(text: &str) -> Result<Vec<Transmitter>, ListError> {
    let mut out = Vec::new();
    let mut at = 0;
    for rec in text.split(['\n', ';']) {
        let start = at;
        at += rec.len() + 1;
        if rec.trim().is_empty() {
            continue;
        }
        let bad = |reason| ListError { at: start, reason };
        let f: Vec<&str> = rec.split(',').map(str::trim).collect();
        if f[0].is_empty() {
            return Err(bad("a record has no label"));
        }
        let freq = f
            .get(1)
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or(bad("a record has no positive frequency in Hz"))?;
        let position = match f.len() {
            2 => None,
            4 => {
                let lat = f[2].parse::<f64>().map_err(|_| bad("the latitude is not a number"))?;
                let lon = f[3].parse::<f64>().map_err(|_| bad("the longitude is not a number"))?;
                Some(
                    Position::checked(lat, lon, 0.0)
                        .map_err(|_| bad("the position is out of range"))?,
                )
            }
            _ => return Err(bad("a record has 2 or 4 fields")),
        };
        if out.len() == MAX_TRANSMITTERS {
            return Err(bad("more than 32 transmitters"));
        }
        out.push(Transmitter {
            label: f[0].to_string(),
            frequency_hz: freq,
            position,
        });
    }
    Ok(out)
}

/// The text of `list` in the form [`parse`] reads: one record per line.
pub fn text_of(list: &[Transmitter]) -> String {
    let mut s = String::new();
    for t in list {
        if !s.is_empty() {
            s.push('\n');
        }
        s.push_str(&t.label);
        s.push(',');
        s.push_str(&csv::number(t.frequency_hz));
        if let Some(p) = t.position {
            s.push(',');
            s.push_str(&csv::number(p.lat_deg));
            s.push(',');
            s.push_str(&csv::number(p.lon_deg));
        }
    }
    s
}

/// The text of the `transmitters` setting made from the rows of a transmitter data file: CSV with a header that names the
/// columns `label` and `frequency_hz`, and optionally `lat` and `lon`; `#` lines are comments. A row without both
/// coordinates gives a record without a position. Rows after the 32nd are left out.
///
/// # Errors
///
/// [`ListError`] if the header lacks `label` or `frequency_hz`, or a row does not parse (the offset is then the row's
/// line number).
pub fn setting_text_from_csv(text: &str) -> Result<String, ListError> {
    let rows = csv::read(text, ',');
    let Some((_, header)) = rows.first() else {
        return Ok(String::new());
    };
    let missing = ListError {
        at: 0,
        reason: "the header names no label or frequency_hz column",
    };
    let label = csv::column(header, &["label"]).ok_or(missing)?;
    let freq = csv::column(header, &["frequency_hz"]).ok_or(missing)?;
    let lat = csv::column(header, &["lat", "latitude"]);
    let lon = csv::column(header, &["lon", "longitude"]);
    let mut list = Vec::new();
    for (line, r) in rows.iter().skip(1).take(MAX_TRANSMITTERS) {
        let cell = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
        let mut rec = format!(
            "{},{}",
            cell(Some(label)).replace([',', ';'], " "),
            cell(Some(freq))
        );
        if !cell(lat).is_empty() && !cell(lon).is_empty() {
            rec.push_str(&format!(",{},{}", cell(lat), cell(lon)));
        }
        let t = parse(&rec).map_err(|e| ListError {
            at: *line,
            reason: e.reason,
        })?;
        list.extend(t);
    }
    Ok(text_of(&list))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_parse_and_print() {
        let l = parse(" A, 24000 ; B,19580,54.9117,-3.2786\n\nC,16400").unwrap();
        assert_eq!(l.len(), 3);
        assert_eq!(l[1].position, Some(Position::new(54.9117, -3.2786)));
        assert_eq!(parse(&text_of(&l)).unwrap(), l);
        assert!(parse("A").is_err());
        assert!(parse("A,0").is_err());
        assert!(parse("A,1000,50").is_err());
        assert!(parse("A,1000,95,0").is_err());
        assert!(parse(&"A,1000;".repeat(33)).is_err());
        assert_eq!(parse("").unwrap(), vec![]);
    }
}
