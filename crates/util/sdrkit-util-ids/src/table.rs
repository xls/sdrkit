//! Delimited text: finding the separator of a list a person pasted or exported from a spreadsheet, and splitting it into records
//! with RFC 4180 quoting (a quoted cell may hold the separator, a doubled quote and a line break).
//!
//! ```
//! use sdrkit_util_ids::table::{records, Separator};
//!
//! let text = "id;name\n232123456;\"Tug; harbour\"\n";
//! let sep = Separator::detect(text);
//! assert_eq!(sep, Separator::Semicolon);
//! let rows: Vec<Vec<String>> = records(text, sep).map(|r| r.cells).collect();
//! assert_eq!(rows[1], ["232123456", "Tug; harbour"]);
//! ```

/// What separates the cells of a line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Separator {
    /// A tab (spreadsheet copy and paste, TSV files).
    Tab,
    /// A comma (CSV).
    Comma,
    /// A semicolon (CSV written where the comma is the decimal mark).
    Semicolon,
    /// A vertical bar.
    Pipe,
    /// An equals sign, one `id=name` pair per line.
    Equals,
    /// Two or more spaces (a column-aligned text file); a single space stays inside a cell.
    Spaces,
}

impl Separator {
    /// A short name for reports: "tab", ",", ";", "|", "=" or "spaces".
    pub fn name(self) -> &'static str {
        match self {
            Separator::Tab => "tab",
            Separator::Comma => ",",
            Separator::Semicolon => ";",
            Separator::Pipe => "|",
            Separator::Equals => "=",
            Separator::Spaces => "spaces",
        }
    }

    /// Finds the separator of a text from its first 50 lines that are neither empty nor `#` comments, outside quotes. A tab wins
    /// when present; otherwise the most frequent of `;`, `|`, `,` and `=` (ties in that order); otherwise runs of spaces.
    pub fn detect(text: &str) -> Separator {
        let mut counts = [0usize; 5]; // tab ; | , =
        for line in text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
            .take(50)
        {
            let mut quoted = false;
            for c in line.chars() {
                match c {
                    '"' => quoted = !quoted,
                    _ if quoted => {}
                    '\t' => counts[0] += 1,
                    ';' => counts[1] += 1,
                    '|' => counts[2] += 1,
                    ',' => counts[3] += 1,
                    '=' => counts[4] += 1,
                    _ => {}
                }
            }
        }
        if counts[0] > 0 {
            return Separator::Tab;
        }
        let order = [
            (Separator::Semicolon, counts[1]),
            (Separator::Pipe, counts[2]),
            (Separator::Comma, counts[3]),
            (Separator::Equals, counts[4]),
        ];
        let mut best = (Separator::Spaces, 0);
        for (s, n) in order {
            if n > best.1 {
                best = (s, n);
            }
        }
        best.0
    }

    fn is(self, c: char) -> bool {
        match self {
            Separator::Tab => c == '\t',
            Separator::Comma => c == ',',
            Separator::Semicolon => c == ';',
            Separator::Pipe => c == '|',
            Separator::Equals => c == '=',
            Separator::Spaces => false,
        }
    }
}

/// One record of a delimited text.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Record {
    /// The line the record starts on, from 1.
    pub line: usize,
    /// The cells, trimmed of surrounding white space, quotes removed and doubled quotes made single.
    pub cells: Vec<String>,
}

impl Record {
    /// Cell `i`, or "" when the record is shorter.
    pub fn cell(&self, i: usize) -> &str {
        self.cells.get(i).map_or("", String::as_str)
    }

    /// Whether every cell is empty.
    pub fn is_blank(&self) -> bool {
        self.cells.iter().all(|c| c.is_empty())
    }
}

/// The records of `text`, skipping empty lines and lines whose first character (after white space) is `#`.
pub fn records(text: &str, sep: Separator) -> Records<'_> {
    Records {
        rest: text,
        line: 1,
        sep,
    }
}

/// The iterator [`records`] returns.
#[derive(Clone, Debug)]
pub struct Records<'a> {
    rest: &'a str,
    line: usize,
    sep: Separator,
}

impl Iterator for Records<'_> {
    type Item = Record;

    fn next(&mut self) -> Option<Record> {
        loop {
            if self.rest.is_empty() {
                return None;
            }
            let start_line = self.line;
            let (rec, used, lines) = split_record(self.rest, self.sep);
            self.rest = &self.rest[used..];
            self.line += lines;
            let first = rec.first().map_or("", String::as_str);
            if rec.iter().all(|c| c.is_empty()) || first.starts_with('#') {
                continue;
            }
            return Some(Record {
                line: start_line,
                cells: rec,
            });
        }
    }
}

/// Splits one record from the front of `text`: the cells, the bytes used (through the line break) and the lines used.
fn split_record(text: &str, sep: Separator) -> (Vec<String>, usize, usize) {
    if sep == Separator::Spaces {
        let end = text.find('\n').map_or(text.len(), |i| i + 1);
        let line = text[..end].trim_end_matches(['\n', '\r']).trim();
        let mut cells = Vec::new();
        let mut rest = line;
        while let Some(i) = rest.find("  ") {
            cells.push(rest[..i].trim().to_string());
            rest = rest[i..].trim_start();
        }
        cells.push(rest.trim().to_string());
        return (cells, end, 1);
    }
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut lines = 1;
    let mut iter = text.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        match c {
            '"' if quoted => {
                if iter.peek().map(|p| p.1) == Some('"') {
                    cur.push('"');
                    iter.next();
                } else {
                    quoted = false;
                }
            }
            '"' if cur.trim().is_empty() && !was_quoted => {
                cur.clear();
                quoted = true;
                was_quoted = true;
            }
            '\n' if !quoted => {
                cells.push(finish(&mut cur, was_quoted));
                return (cells, i + 1, lines);
            }
            '\r' if !quoted => {}
            c if !quoted && sep.is(c) => {
                cells.push(finish(&mut cur, was_quoted));
                was_quoted = false;
            }
            c => {
                if c == '\n' {
                    lines += 1;
                }
                cur.push(c);
            }
        }
    }
    cells.push(finish(&mut cur, was_quoted));
    (cells, text.len(), lines)
}

fn finish(cur: &mut String, was_quoted: bool) -> String {
    let s = std::mem::take(cur);
    if was_quoted {
        // Keep the inside of the quotes as written; only text after the closing quote is trimmed away.
        s.trim_end().to_string()
    } else {
        s.trim().to_string()
    }
}

/// Writes one cell for a tab-separated export: tabs and line breaks become spaces, and a cell a spreadsheet would read as a
/// formula (starting with `=`, `+`, `-` or `@`, CWE-1236) gets a leading apostrophe. Numbers with a sign are left alone.
pub fn export_cell(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| {
            if matches!(c, '\t' | '\n' | '\r') {
                ' '
            } else {
                c
            }
        })
        .collect();
    let numeric = clean.parse::<f64>().is_ok();
    if !numeric && clean.starts_with(['=', '+', '-', '@']) {
        format!("'{clean}")
    } else {
        clean
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(text: &str, sep: Separator) -> Vec<Vec<String>> {
        records(text, sep).map(|r| r.cells).collect()
    }

    #[test]
    fn detects_separators() {
        assert_eq!(Separator::detect("a\tb\nc\td"), Separator::Tab);
        assert_eq!(Separator::detect("a;b;c\n1;2;3"), Separator::Semicolon);
        assert_eq!(Separator::detect("a,b\n1,2"), Separator::Comma);
        assert_eq!(Separator::detect("a|b"), Separator::Pipe);
        assert_eq!(
            Separator::detect("91=Worldwide\n92=Europe"),
            Separator::Equals
        );
        assert_eq!(
            Separator::detect("232123456  MV Example"),
            Separator::Spaces
        );
        // Separators inside quotes do not count.
        assert_eq!(
            Separator::detect("\"a,b,c,d\";x\n1;2"),
            Separator::Semicolon
        );
        // Comments are not read.
        assert_eq!(Separator::detect("# a,b,c,d\n1;2"), Separator::Semicolon);
    }

    #[test]
    fn rfc4180_quoting() {
        let r = rows(
            "a,\"b,c\",\"d \"\"e\"\"\"\n\"multi\nline\",x\n",
            Separator::Comma,
        );
        assert_eq!(r[0], ["a", "b,c", "d \"e\""]);
        assert_eq!(r[1], ["multi\nline", "x"]);
        let lines: Vec<usize> = records("a\n\"x\ny\",1\nb", Separator::Comma)
            .map(|r| r.line)
            .collect();
        assert_eq!(lines, [1, 2, 4]);
    }

    #[test]
    fn comments_blanks_and_crlf() {
        let r = rows("# head\r\n\r\n1;x\r\n  \r\n2;y", Separator::Semicolon);
        assert_eq!(r, vec![vec!["1", "x"], vec!["2", "y"]]);
        let r = rows("232123456  MV  Example  Tug\n", Separator::Spaces);
        assert_eq!(r[0], ["232123456", "MV", "Example", "Tug"]);
        let r = rows("232123456  MV Example", Separator::Spaces);
        assert_eq!(r[0], ["232123456", "MV Example"]);
    }

    #[test]
    fn export_guards_formulas() {
        assert_eq!(export_cell("=cmd|' /C calc'!A0"), "'=cmd|' /C calc'!A0");
        assert_eq!(export_cell("-12.5"), "-12.5");
        assert_eq!(export_cell("a\tb\nc"), "a b c");
        assert_eq!(export_cell("@SUM(1)"), "'@SUM(1)");
    }
}
