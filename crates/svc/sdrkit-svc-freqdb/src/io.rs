//! Import and export of entries as CSV (the columns of [`CSV_COLUMNS`], the header of a frequency manager, or a plain
//! `frequency,name` list) and JSON (an array of entries, `{"entries": [...]}`, or the memory file). A settings cell is a JSON
//! list of settings with their owners.

use crate::db::{parse_frequency, validate, FreqDb};
use crate::entry::{Category, Entry};
use crate::error::{DbError, LineProblem, Problem};
use sdrkit_core::OwnerSetting;
use sdrkit_util_text::csv;
use serde_json::Value;

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ImportReport {
    /// The ids of the entries added.
    pub added: Vec<String>,
    /// The rows skipped because the same frequency and name is already an entry of the user.
    pub duplicates: usize,
    /// The rows not added because the user list was full.
    pub over_cap: usize,
    /// The rows that could not be read.
    pub problems: Vec<LineProblem>,
}

/// Rows read from a file: each entry with its starred flag, and the rows that could not be read.
pub(crate) type Parsed = (Vec<(Entry, bool)>, Vec<LineProblem>);

/// The CSV columns written.
pub(crate) const CSV_COLUMNS: [&str; 13] = [
    "name",
    "frequency_hz",
    "mode",
    "submode",
    "bandwidth_hz",
    "group",
    "tags",
    "notes",
    "starred",
    "settings",
    "end_hz",
    "scope",
    "source",
];

fn number(v: f64) -> String {
    csv::number(v)
}

/// Entries as CSV, with the starred flag from `db`.
pub(crate) fn to_csv(db: &FreqDb, entries: &[&Entry]) -> String {
    let mut out = csv::record(&CSV_COLUMNS, ',');
    out.push('\n');
    for e in entries {
        out.push_str(&csv::record(
            &[
                e.name.clone(),
                number(e.hz),
                e.stack.clone(),
                e.submode.clone(),
                e.bandwidth_hz.map(number).unwrap_or_default(),
                e.category.id().to_string(),
                e.tags.join(";"),
                e.notes.clone(),
                if db.is_starred(&e.id) {
                    "1".into()
                } else {
                    String::new()
                },
                if e.settings.is_empty() {
                    String::new()
                } else {
                    serde_json::to_string(&e.settings).unwrap_or_default()
                },
                e.end_hz.map(number).unwrap_or_default(),
                e.scope.join(" "),
                e.source.clone(),
            ],
            ',',
        ));
        out.push('\n');
    }
    out
}

/// Entries as JSON: an array, with the starred flag added.
pub(crate) fn to_json(db: &FreqDb, entries: &[&Entry]) -> String {
    let v: Vec<Value> = entries
        .iter()
        .map(|e| {
            let mut v = serde_json::to_value(e).unwrap_or(Value::Null);
            if let Value::Object(m) = &mut v {
                m.insert("starred".into(), Value::Bool(db.is_starred(&e.id)));
            }
            v
        })
        .collect();
    serde_json::to_string_pretty(&v).unwrap_or_else(|_| "[]".into())
}

#[derive(Clone, Copy, PartialEq)]
enum Col {
    Name,
    Hz,
    Freq(f64),
    FreqGuess,
    Mode,
    Submode,
    Bandwidth(f64),
    Category,
    Tags,
    Notes,
    Starred,
    Settings,
    End,
    Scope,
    Source,
}

fn column(h: &str) -> Option<Col> {
    let h = h.trim().to_ascii_lowercase();
    let unit = if h.contains("mhz") {
        Some(1e6)
    } else if h.contains("khz") {
        Some(1e3)
    } else if h.contains("hz") {
        Some(1.0)
    } else {
        None
    };
    let base = h.split(['(', '[', ' ']).next().unwrap_or("").trim();
    Some(match base {
        "frequency_hz" | "freq_hz" | "hz" => Col::Hz,
        "frequency" | "freq" | "freq." | "mhz" | "khz" | "rx" | "rx_freq" | "receive" => {
            let u = match base {
                "mhz" => Some(1e6),
                "khz" => Some(1e3),
                _ => unit,
            };
            u.map(Col::Freq).unwrap_or(Col::FreqGuess)
        }
        "name" | "description" | "desc" | "label" | "station" | "alpha" | "title" | "tag" => {
            Col::Name
        }
        "mode" | "modulation" | "demodulator" | "demod" => Col::Mode,
        "submode" => Col::Submode,
        "bandwidth_hz" => Col::Bandwidth(1.0),
        "bandwidth" | "bw" | "filter" => Col::Bandwidth(unit.unwrap_or(1.0)),
        "group" | "category" | "folder" | "service" | "bank" => Col::Category,
        "tags" => Col::Tags,
        "notes" | "note" | "comment" | "comments" | "remarks" => Col::Notes,
        "starred" | "favorite" | "favourite" | "star" => Col::Starred,
        "settings" => Col::Settings,
        "end_hz" => Col::End,
        "scope" => Col::Scope,
        "source" => Col::Source,
        _ => return None,
    })
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "x" | "*"
    )
}

/// The modulation names of other programs, as the demodulators of the general receiver.
fn demod_of(mode: &str) -> Option<&'static str> {
    Some(match mode.trim().to_ascii_lowercase().as_str() {
        "nfm" | "fm" | "fmn" | "nbfm" => "nfm",
        "wfm" | "wbfm" | "fmw" => "wfm",
        "am" | "dsb" => "am",
        "usb" => "usb",
        "lsb" => "lsb",
        "cw" | "cwu" | "cwl" | "cw-u" | "cw-l" => "cw",
        _ => return None,
    })
}

/// Sets the stack and the submode from a mode cell: a demodulator name becomes the general receiver, anything else a stack id.
fn set_mode(e: &mut Entry, mode: &str) {
    let m = mode.trim();
    if m.is_empty() {
        return;
    }
    if let Some(d) = demod_of(m) {
        e.stack = "general".into();
        e.submode = d.into();
    } else {
        e.stack = m.to_ascii_lowercase();
    }
}

fn set_scope(e: &mut Entry, text: &str) {
    e.scope.extend(
        text.split([' ', ';', ','])
            .filter(|p| !p.is_empty())
            .map(String::from),
    );
}

/// Reads CSV with a header, or a plain `frequency,name` list, into entries with their starred flags. Each row is checked.
pub(crate) fn parse_csv(text: &str) -> Parsed {
    let delim = csv::sniff_delimiter(text);
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let mut cols: Option<Vec<Option<Col>>> = None;
    let mut first = true;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = csv::split_line(line, delim);
        if first {
            first = false;
            let mapped: Vec<Option<Col>> = fields.iter().map(|f| column(f)).collect();
            if mapped
                .iter()
                .any(|c| matches!(c, Some(Col::Hz | Col::Freq(_) | Col::FreqGuess)))
            {
                cols = Some(mapped);
                continue;
            }
        }
        let line_no = i + 1;
        let mut e = Entry::new("", "", 0.0, Category::Other);
        let mut starred = false;
        match &cols {
            None => {
                // A plain list: frequency, name.
                let Some(hz) = fields.first().and_then(|f| parse_frequency(f)) else {
                    errors.push(LineProblem {
                        line: line_no,
                        kind: Problem::NoFrequency,
                    });
                    continue;
                };
                e.hz = hz;
                e.name = fields.get(1).cloned().unwrap_or_default();
                if e.name.is_empty() {
                    e.name = sdrkit_data_plans::format_frequency(hz);
                }
            }
            Some(cols) => {
                let mut bad = None;
                for (c, v) in cols.iter().zip(&fields) {
                    let v = v.trim();
                    match c {
                        Some(Col::Name) => e.name = v.into(),
                        Some(Col::Hz) => e.hz = v.parse().unwrap_or(0.0),
                        Some(Col::Freq(u)) => {
                            e.hz = v
                                .parse::<f64>()
                                .map(|x| x * u)
                                .ok()
                                .or_else(|| parse_frequency(v))
                                .unwrap_or(0.0)
                        }
                        Some(Col::FreqGuess) => e.hz = parse_frequency(v).unwrap_or(0.0),
                        Some(Col::Mode) => set_mode(&mut e, v),
                        Some(Col::Submode) if !v.is_empty() => e.submode = v.to_ascii_lowercase(),
                        Some(Col::Bandwidth(u)) if !v.is_empty() => {
                            e.bandwidth_hz = v.parse::<f64>().ok().map(|x| x * u)
                        }
                        // A folder name that is not a category is kept as a tag.
                        Some(Col::Category) if !v.is_empty() => match Category::from_id(v) {
                            Some(c) => e.category = c,
                            None => e.tags.push(v.to_string()),
                        },
                        Some(Col::Tags) => e.tags.extend(
                            v.split([';', '|'])
                                .map(str::trim)
                                .filter(|t| !t.is_empty())
                                .map(String::from),
                        ),
                        Some(Col::Notes) => e.notes = v.into(),
                        Some(Col::Starred) => starred = truthy(v),
                        Some(Col::Settings) if !v.is_empty() => {
                            match serde_json::from_str::<Vec<OwnerSetting>>(v) {
                                Ok(list) => e.settings = list,
                                Err(_) => bad = Some(Problem::NotSettings),
                            }
                        }
                        Some(Col::End) if !v.is_empty() => e.end_hz = v.parse().ok(),
                        Some(Col::Scope) => set_scope(&mut e, v),
                        Some(Col::Source) => e.source = v.into(),
                        _ => {}
                    }
                }
                if let Some(kind) = bad {
                    errors.push(LineProblem {
                        line: line_no,
                        kind,
                    });
                    continue;
                }
                if e.name.is_empty() && e.hz > 0.0 {
                    e.name = sdrkit_data_plans::format_frequency(e.hz);
                }
            }
        }
        match validate(&e) {
            Ok(()) => out.push((e, starred)),
            Err(err) => errors.push(LineProblem {
                line: line_no,
                kind: problem_of(&err),
            }),
        }
    }
    (out, errors)
}

/// The problem of a refused entry.
fn problem_of(e: &DbError) -> Problem {
    match e {
        DbError::Invalid(p) => *p,
        _ => Problem::Full,
    }
}

/// Reads JSON: an array of entries, `{"entries": [...]}`, or a memory file (`{"memories": [...]}`). A text that is not one of
/// these returns one problem at line 1. Items count from 1.
pub(crate) fn parse_json(text: &str) -> Parsed {
    let not_json = || {
        (
            Vec::new(),
            vec![LineProblem {
                line: 1,
                kind: Problem::NotJson,
            }],
        )
    };
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return not_json();
    };
    let list = match &v {
        Value::Array(a) => a,
        Value::Object(m) => match m
            .get("entries")
            .or_else(|| m.get("memories"))
            .and_then(Value::as_array)
        {
            Some(a) => a,
            None => return not_json(),
        },
        _ => return not_json(),
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, item) in list.iter().enumerate() {
        let line = i + 1;
        let Value::Object(m) = item else {
            errors.push(LineProblem {
                line,
                kind: Problem::NotAnEntry,
            });
            continue;
        };
        let s = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let f = |k: &str| m.get(k).and_then(Value::as_f64);
        let hz = f("hz")
            .or_else(|| f("freq_hz"))
            .or_else(|| f("frequency_hz"))
            .unwrap_or(0.0);
        let mut e = Entry::new("", s("name"), hz, Category::Other);
        // A group of a memory that is not a category is kept as a tag.
        for (key, tag) in [("category", false), ("group", true)] {
            let text = s(key);
            match Category::from_id(&text) {
                Some(c) => e.category = c,
                None if tag && !text.is_empty() => e.tags.push(text),
                None => {}
            }
        }
        e.end_hz = f("end_hz");
        e.bandwidth_hz = f("bandwidth_hz");
        e.stack = s("stack");
        e.submode = s("submode");
        if let Some(Value::Array(t)) = m.get("tags") {
            e.tags
                .extend(t.iter().filter_map(Value::as_str).map(String::from));
        }
        if let Some(Value::Array(c)) = m.get("scope") {
            e.scope = c
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect();
        }
        e.notes = s("notes");
        e.source = s("source");
        if let Some(st) = m.get("settings") {
            match serde_json::from_value::<Vec<OwnerSetting>>(st.clone()) {
                Ok(list) => e.settings = list,
                Err(_) => {
                    errors.push(LineProblem {
                        line,
                        kind: Problem::NotSettings,
                    });
                    continue;
                }
            }
        }
        let starred = m.get("starred").and_then(Value::as_bool).unwrap_or(false);
        match validate(&e) {
            Ok(()) => out.push((e, starred)),
            Err(err) => errors.push(LineProblem {
                line,
                kind: problem_of(&err),
            }),
        }
    }
    (out, errors)
}

/// Imports text as entries of the user: JSON if it starts with `{` or `[`, else CSV. Duplicates of entries of the user (the
/// same frequency and name) are skipped.
pub(crate) fn import(db: &mut FreqDb, text: &str) -> ImportReport {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    let (rows, problems) = if t.starts_with('{') || t.starts_with('[') {
        parse_json(t)
    } else {
        parse_csv(t)
    };
    let mut report = ImportReport {
        problems,
        ..Default::default()
    };
    for (e, starred) in rows {
        if db.has_user_duplicate(&e) {
            report.duplicates += 1;
            continue;
        }
        match db.add_user(e) {
            Ok(id) => {
                if starred {
                    db.star(&id, true);
                }
                report.added.push(id);
            }
            Err(_) => report.over_cap += 1,
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_round_trip() {
        let mut db = FreqDb::new();
        let mut e = Entry::new("", "Club, \"North\"", 145.6125e6, Category::Amateur);
        e.stack = "general".into();
        e.submode = "nfm".into();
        e.bandwidth_hz = Some(12500.0);
        e.tags = vec!["repeater".into(), "local".into()];
        e.scope = vec!["itu1".into(), "DE".into()];
        e.settings.push(OwnerSetting {
            owner: sdrkit_core::OwnerKey::of("stack.general"),
            key: "squelch".into(),
            value: sdrkit_core::Value::Choice(1),
        });
        let id = db.add_user(e).unwrap();
        db.star(&id, true);
        let user: Vec<&Entry> = db.user_entries().iter().collect();
        let text = to_csv(&db, &user);
        let mut other = FreqDb::new();
        let r = import(&mut other, &text);
        assert_eq!(
            (r.added.len(), r.problems.len()),
            (1, 0),
            "{:?}",
            r.problems
        );
        let back = &other.user_entries()[0];
        assert_eq!(back.name, "Club, \"North\"");
        assert_eq!(back.hz, 145.6125e6);
        assert_eq!(
            (back.stack.as_str(), back.submode.as_str()),
            ("general", "nfm")
        );
        assert_eq!(back.tags, vec!["repeater", "local"]);
        assert_eq!(back.category, Category::Amateur);
        assert_eq!(back.scope, vec!["itu1", "DE"]);
        assert_eq!(back.settings[0].value, sdrkit_core::Value::Choice(1));
        assert!(other.is_starred(&r.added[0]));
        // Again: all duplicates.
        let r = import(&mut other, &text);
        assert_eq!((r.added.len(), r.duplicates), (0, 1));
    }

    #[test]
    fn other_layouts() {
        let mut db = FreqDb::new();
        let plain = "145.500,Calling\n7074 kHz,FT8\nbad,line\n";
        let r = import(&mut db, plain);
        assert_eq!(r.added.len(), 2);
        assert_eq!(
            r.problems,
            vec![LineProblem {
                line: 3,
                kind: Problem::NoFrequency
            }]
        );
        let mgr = "Frequency (MHz);Description;Mode;Bandwidth (kHz);Comment\n118.700;Tower;AM;8.33;runway 27\n";
        let r = import(&mut db, mgr);
        assert_eq!(r.added.len(), 1, "{:?}", r.problems);
        let e = db.get(&r.added[0]).unwrap();
        assert_eq!(
            (e.hz, e.submode.as_str(), e.notes.as_str()),
            (118.7e6, "am", "runway 27")
        );
        assert!((e.bandwidth_hz.unwrap() - 8330.0).abs() < 1.0);
        let memories = r#"{"version":1,"memories":[{"id":4,"name":"Local FM","freq_hz":99.5e6,"stack":"fm","submode":"",
            "tags":["music"],"group":"Broadcast","starred":true,
            "settings":[{"owner":"stack.fm","key":"band","value":{"kind":"choice","value":1}}]}]}"#;
        let r = import(&mut db, memories);
        let e = db.get(&r.added[0]).unwrap();
        assert_eq!((e.category, e.stack.as_str()), (Category::Broadcast, "fm"));
        assert!(db.is_starred(&e.id));
        assert_eq!(e.settings[0].key, "band");
        let all: Vec<&Entry> = db.user_entries().iter().collect();
        let json = to_json(&db, &all);
        let mut again = FreqDb::new();
        assert_eq!(import(&mut again, &json).added.len(), 4);
        assert_eq!(import(&mut again, "{").problems[0].kind, Problem::NotJson);
    }
}
