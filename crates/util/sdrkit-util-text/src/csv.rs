//! A CSV reader and writer with RFC 4180 quoting: quoted fields with doubled quotes, line breaks inside quotes, `,`, `;` or tab
//! as the delimiter, a byte-order mark and `#` comment lines skipped.
//!
//! Standard: IETF RFC 4180 clause 2

/// Returns the delimiter of a CSV text, guessed from its first line that is not empty and not a comment: the most frequent of
/// `,`, `;` and tab outside quotes, or a comma if none appears.
pub fn sniff_delimiter(text: &str) -> char {
    let line = text
        .trim_start_matches('\u{feff}')
        .lines()
        .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .unwrap_or("");
    let mut counts = [0usize; 3];
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => counts[0] += 1,
            ';' if !quoted => counts[1] += 1,
            '\t' if !quoted => counts[2] += 1,
            _ => {}
        }
    }
    let best = (0..3).max_by_key(|&i| (counts[i], 3 - i)).unwrap_or(0);
    if counts[best] == 0 {
        ','
    } else {
        [',', ';', '\t'][best]
    }
}

/// A field as read: its text, and true if it was quoted.
struct Field {
    text: String,
    quoted: bool,
}

/// Reads the records of `text` and calls `emit` with the line each record starts on (from 1) and its fields. If `comments`
/// is true, a line that starts with `#` is skipped.
fn scan(text: &str, delimiter: char, comments: bool, mut emit: impl FnMut(usize, Vec<Field>)) {
    let text = text.trim_start_matches('\u{feff}');
    let mut record: Vec<Field> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut line = 1usize;
    let mut record_line = 1usize;
    let mut chars = text.chars().peekable();
    let mut at_record_start = true;
    while let Some(c) = chars.next() {
        if at_record_start {
            record_line = line;
            if comments && c == '#' {
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
                line += 1;
                continue;
            }
            at_record_start = false;
        }
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                if c == '\n' {
                    line += 1;
                }
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
                was_quoted = true;
            }
            '\r' => {}
            '\n' => {
                line += 1;
                record.push(Field {
                    text: std::mem::take(&mut field),
                    quoted: std::mem::take(&mut was_quoted),
                });
                emit(record_line, std::mem::take(&mut record));
                at_record_start = true;
            }
            c if c == delimiter => {
                record.push(Field {
                    text: std::mem::take(&mut field),
                    quoted: std::mem::take(&mut was_quoted),
                });
            }
            c => field.push(c),
        }
    }
    if !at_record_start {
        record.push(Field {
            text: field,
            quoted: was_quoted,
        });
        emit(record_line, record);
    }
}

/// Reads the records of `text`, separated by `delimiter`. Returns each record with the line it starts on (from 1). Empty
/// records and lines that start with `#` are skipped. Fields keep their spaces.
///
/// ```
/// use sdrkit_util_text::csv;
/// let rows = csv::read("name,freq\n\"Tower, main\",118.1\n", ',');
/// assert_eq!(rows[1], (2, vec!["Tower, main".to_string(), "118.1".to_string()]));
/// ```
pub fn read(text: &str, delimiter: char) -> Vec<(usize, Vec<String>)> {
    let mut out = Vec::new();
    scan(text, delimiter, true, |line, record| {
        if record.iter().any(|f| !f.text.trim().is_empty()) {
            out.push((line, record.into_iter().map(|f| f.text).collect()));
        }
    });
    out
}

/// Splits one line into its fields, separated by `delimiter`. A quoted field keeps its text as written. An unquoted field
/// is trimmed. An empty line returns one empty field.
pub fn split_line(line: &str, delimiter: char) -> Vec<String> {
    let mut out = Vec::new();
    scan(line, delimiter, false, |_, record| {
        if out.is_empty() {
            out = record
                .into_iter()
                .map(|f| {
                    if f.quoted {
                        f.text
                    } else {
                        f.text.trim().to_string()
                    }
                })
                .collect();
        }
    });
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Returns true if `value` must be quoted: it holds the delimiter, a quote or a line break, or starts or ends with a space.
fn needs_quotes(value: &str, delimiter: char) -> bool {
    value.contains([delimiter, '"', '\n', '\r']) || value.starts_with(' ') || value.ends_with(' ')
}

/// Appends `value` to `out` as one field, quoted if it must be.
pub fn push_field(out: &mut String, value: &str, delimiter: char) {
    if needs_quotes(value, delimiter) {
        out.push('"');
        for c in value.chars() {
            if c == '"' {
                out.push('"');
            }
            out.push(c);
        }
        out.push('"');
    } else {
        out.push_str(value);
    }
}

/// Appends one record to `out`, without the line end.
pub fn push_record<'a>(
    out: &mut String,
    fields: impl IntoIterator<Item = &'a str>,
    delimiter: char,
) {
    for (i, f) in fields.into_iter().enumerate() {
        if i > 0 {
            out.push(delimiter);
        }
        push_field(out, f, delimiter);
    }
}

/// Returns `value` as one field, quoted if it must be.
pub fn field(value: &str, delimiter: char) -> String {
    let mut out = String::new();
    push_field(&mut out, value, delimiter);
    out
}

/// Returns one record, without the line end.
pub fn record<S: AsRef<str>>(fields: &[S], delimiter: char) -> String {
    let mut out = String::new();
    push_record(&mut out, fields.iter().map(AsRef::as_ref), delimiter);
    out
}

/// Returns `v` as text with at most nine decimals and no trailing zeros ("8", "474", "446.00625").
pub fn number(v: f64) -> String {
    let s = format!("{v:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.into()
    }
}

/// Returns the index of the header column whose name is one of `names`, compared on letters and digits only and without
/// case.
pub fn column(header: &[String], names: &[&str]) -> Option<usize> {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    };
    header
        .iter()
        .position(|h| names.iter().any(|n| norm(h) == norm(n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_quotes_comments_and_line_breaks() {
        let text = "\u{feff}# a comment\nname,freq\n\"Tower, main\",118.1\n\"say \"\"hi\"\"\",\"two\nlines\"\n\n,\n";
        let rows = read(text, ',');
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], (2, vec!["name".into(), "freq".into()]));
        assert_eq!(rows[1].1, vec!["Tower, main", "118.1"]);
        assert_eq!(rows[2].1, vec!["say \"hi\"", "two\nlines"]);
    }

    #[test]
    fn round_trips_awkward_fields() {
        let fields = ["a,b", "q\"q", " lead", "plain", "x\ny", ""];
        let line = record(&fields, ',');
        assert_eq!(read(&line, ',')[0].1, fields);
        let one = record(&fields[..4], ';');
        assert_eq!(split_line(&one, ';'), fields[..4]);
    }

    #[test]
    fn splits_one_line_and_trims_unquoted_fields() {
        assert_eq!(split_line(" 1 , two ,3", ','), vec!["1", "two", "3"]);
        assert_eq!(split_line("a, \" b \",", ','), vec!["a", " b ", ""]);
        assert_eq!(split_line("", ','), vec![""]);
        assert_eq!(split_line("#x,y", ','), vec!["#x", "y"]);
    }

    #[test]
    fn writes_rfc_4180_quoting() {
        let mut s = String::new();
        push_record(&mut s, ["aaa", "b\r\nbb", "ccc"], ',');
        assert_eq!(s, "aaa,\"b\r\nbb\",ccc");
        assert_eq!(field("b\"bb", ','), "\"b\"\"bb\"");
        assert_eq!(field("a;b", ','), "a;b");
        assert_eq!(field("a;b", ';'), "\"a;b\"");
    }

    #[test]
    fn sniffs_the_delimiter() {
        assert_eq!(sniff_delimiter("a;b;c\n1;2;3"), ';');
        assert_eq!(sniff_delimiter("a\tb\n"), '\t');
        assert_eq!(sniff_delimiter("\"a;b\",c\n"), ',');
        assert_eq!(sniff_delimiter("single"), ',');
    }

    #[test]
    fn numbers_and_columns() {
        assert_eq!(number(474e6 / 1e6), "474");
        assert_eq!(number(446.00625), "446.00625");
        assert_eq!(number(-0.0), "0");
        let h: Vec<String> = ["Centre MHz", "Name"].map(String::from).to_vec();
        assert_eq!(column(&h, &["centre_mhz"]), Some(0));
    }
}
