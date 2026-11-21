//! Id-to-name lists a person keeps: ship names by MMSI, aircraft by ICAO address, pager capcodes, talkgroups and radio ids, SELCAL
//! codes. One importer reads them all: it finds the separator ([`Separator::detect`]), finds a header row and the id and name
//! columns from their titles, cleans the ids the way spreadsheets damage them (an apostrophe in front, the leading zeros of an
//! MMSI cut off), and reports every row it refused with the line and the reason.
//!
//! Merge rule: the id is the key; a non-empty cell replaces the stored value and an empty cell never erases one. Use
//! [`IdNames::remove`] to delete.
//!
//! ```
//! use sdrkit_util_ids::names::{IdKind, IdNames, Rejection};
//!
//! let text = "mmsi;name;callsign\n'2320064;Solent Coastguard;\n232123456;MV Example;MABC2\nnot a number;Bad\n";
//! let (names, report) = IdNames::from_text(IdKind::Mmsi, text);
//! assert_eq!(names.name("002320064"), Some("Solent Coastguard"));
//! assert_eq!(names.get("232123456").and_then(|e| e.field("callsign")), Some("MABC2"));
//! assert_eq!(report.added, 2);
//! assert_eq!(report.rejected[0].line, 4);
//! assert_eq!(report.rejected[0].reason, Rejection::BadId);
//! ```

use crate::table::{export_cell, records, Separator};
use std::collections::BTreeMap;
use std::fmt;

/// The longest name kept, in characters; longer names are cut.
pub const MAX_NAME_CHARS: usize = 120;
/// The most rejected rows a report lists (it still counts them all).
pub const MAX_REJECTED_LISTED: usize = 200;

/// What kind of id a list holds; it decides how an id cell is read and written.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IdKind {
    /// A decimal number (talkgroup, radio id, pager capcode, unit id); leading zeros are dropped. A range `a-b` is allowed.
    Decimal,
    /// An MMSI: nine digits. Seven or eight digits get back the leading zeros a spreadsheet removed (coast and group stations
    /// start with 0, ITU-R M.585).
    Mmsi,
    /// An ICAO 24-bit aircraft address: six hex digits, written upper case. A range `a-b` is allowed.
    Icao,
    /// A SELCAL code: four tone designators (16-tone or 32-tone set), written `AB-CD`.
    Selcal,
    /// Any text, compared without case and with runs of white space made single.
    Text,
}

/// A normalised id: a number for the numeric kinds, upper-case text for the others.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Key {
    Num(u64),
    Text(String),
}

impl IdKind {
    fn numeric(self) -> bool {
        matches!(self, IdKind::Decimal | IdKind::Mmsi | IdKind::Icao)
    }

    fn ranges(self) -> bool {
        matches!(self, IdKind::Decimal | IdKind::Icao)
    }

    /// Reads one id. Spreadsheet damage is undone: white space and a leading apostrophe are removed.
    fn key(self, cell: &str) -> Option<Key> {
        let s: String = cell
            .trim()
            .trim_start_matches('\'')
            .chars()
            .filter(|c| !c.is_whitespace() || self == IdKind::Text)
            .collect();
        if s.is_empty() {
            return None;
        }
        match self {
            IdKind::Decimal => {
                if !s.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                s.parse::<u64>().ok().map(Key::Num)
            }
            IdKind::Mmsi => {
                if !(7..=9).contains(&s.len()) || !s.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                s.parse::<u64>().ok().map(Key::Num)
            }
            IdKind::Icao => {
                let h = s
                    .strip_prefix("0x")
                    .or_else(|| s.strip_prefix("0X"))
                    .unwrap_or(&s);
                if h.is_empty() || h.len() > 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                u64::from_str_radix(h, 16).ok().map(Key::Num)
            }
            IdKind::Selcal => {
                crate::tones::selcal::Code::parse(&s).map(|c| Key::Text(c.to_string()))
            }
            IdKind::Text => Some(Key::Text(
                s.split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_uppercase(),
            )),
        }
    }

    /// Reads an id or, for [`IdKind::Decimal`] and [`IdKind::Icao`], a range `a-b` (either order).
    fn key_or_range(self, cell: &str) -> Option<Target> {
        if self.ranges() {
            if let Some((a, b)) = cell.trim().trim_start_matches('\'').split_once('-') {
                let (Some(Key::Num(a)), Some(Key::Num(b))) = (self.key(a), self.key(b)) else {
                    return None;
                };
                return Some(if a == b {
                    Target::One(Key::Num(a))
                } else {
                    Target::Range(a.min(b), a.max(b))
                });
            }
        }
        self.key(cell).map(Target::One)
    }

    fn write(self, key: &Key, out: &mut String) {
        use fmt::Write as _;
        let _ = match (self, key) {
            (IdKind::Mmsi, Key::Num(n)) => write!(out, "{n:09}"),
            (IdKind::Icao, Key::Num(n)) => write!(out, "{n:06X}"),
            (_, Key::Num(n)) => write!(out, "{n}"),
            (_, Key::Text(t)) => write!(out, "{t}"),
        };
    }

    fn write_range(self, a: u64, b: u64, out: &mut String) {
        self.write(&Key::Num(a), out);
        out.push('-');
        self.write(&Key::Num(b), out);
    }

    /// Returns the normal written form of an id (`"2320064"` as an MMSI becomes `"002320064"`), or `None` when it is not one.
    ///
    /// ```
    /// use sdrkit_util_ids::names::IdKind;
    /// assert_eq!(IdKind::Icao.normalise("0x4ca123").as_deref(), Some("4CA123"));
    /// assert_eq!(IdKind::Selcal.normalise("am cp").as_deref(), Some("AM-CP"));
    /// assert_eq!(IdKind::Decimal.normalise("0091").as_deref(), Some("91"));
    /// ```
    pub fn normalise(self, id: &str) -> Option<String> {
        let k = self.key(id)?;
        let mut s = String::new();
        self.write(&k, &mut s);
        Some(s)
    }
}

enum Target {
    One(Key),
    Range(u64, u64),
}

/// One entry: a name and the other columns of its row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// The name ("" when the list provides only other columns).
    pub name: String,
    /// The other columns, by column title (lower case) or `column3`, `column4` ... when the list has no header.
    pub fields: Vec<(String, String)>,
}

impl Entry {
    /// The value of a column, by title (case is ignored).
    pub fn field(&self, title: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(title))
            .map(|(_, v)| v.as_str())
    }

    /// Merges `other` in: its non-empty values replace ours. Returns whether anything changed.
    fn merge(&mut self, other: Entry) -> bool {
        let mut changed = false;
        if !other.name.is_empty() && self.name != other.name {
            self.name = other.name;
            changed = true;
        }
        for (k, v) in other.fields {
            if v.is_empty() {
                continue;
            }
            match self.fields.iter_mut().find(|(sk, _)| *sk == k) {
                Some((_, sv)) if *sv == v => {}
                Some((_, sv)) => {
                    *sv = v;
                    changed = true;
                }
                None => {
                    self.fields.push((k, v));
                    changed = true;
                }
            }
        }
        changed
    }

    fn is_empty(&self) -> bool {
        self.name.is_empty() && self.fields.iter().all(|(_, v)| v.is_empty())
    }
}

/// Why a row was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rejection {
    /// The id cell is empty or not an id of the list's kind.
    BadId,
    /// The row has an id but no name and no other value.
    Empty,
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Rejection::BadId => "not a valid id",
            Rejection::Empty => "no name or other value",
        })
    }
}

/// A refused row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    /// The line the row starts on, from 1.
    pub line: usize,
    /// Why.
    pub reason: Rejection,
    /// The id cell as written (at most 40 characters).
    pub id: String,
}

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// The separator found.
    pub separator: Option<Separator>,
    /// Whether the first row was a header.
    pub header: bool,
    /// The column titles in use (from the header, or `id`, `name`, `column3` ... without one).
    pub columns: Vec<String>,
    /// Rows read as entries.
    pub read: usize,
    /// Entries new to the list.
    pub added: usize,
    /// Entries that changed.
    pub changed: usize,
    /// Rows refused, in total.
    pub rejected_count: usize,
    /// The first [`MAX_REJECTED_LISTED`] refused rows.
    pub rejected: Vec<Rejected>,
}

/// The titles taken as the id column, compared after removing case, spaces, `_`, `-` and `.`.
const ID_TITLES: &[&str] = &[
    "id",
    "mmsi",
    "icao",
    "icao24",
    "hex",
    "address",
    "modes",
    "modescode",
    "capcode",
    "ric",
    "radioid",
    "rid",
    "talkgroup",
    "tg",
    "tgid",
    "selcal",
    "code",
    "key",
    "unit",
    "unitid",
];
/// The column titles accepted as the name column, in order of preference.
const NAME_TITLES: &[&str] = &[
    "name",
    "alias",
    "label",
    "shipname",
    "vessel",
    "station",
    "registration",
    "reg",
    "callsign",
    "call",
    "description",
    "desc",
];

fn title_key(cell: &str) -> String {
    cell.chars()
        .filter(|c| !matches!(c, ' ' | '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn clean_name(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_NAME_CHARS).collect()
}

/// An id-to-name list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdNames {
    kind: IdKind,
    single: BTreeMap<Key, Entry>,
    ranges: Vec<(u64, u64, Entry)>,
    version: u64,
}

impl IdNames {
    /// An empty list of ids of `kind`.
    pub fn new(kind: IdKind) -> IdNames {
        IdNames {
            kind,
            single: BTreeMap::new(),
            ranges: Vec::new(),
            version: 0,
        }
    }

    /// A list read from `text`, and the report of the import.
    pub fn from_text(kind: IdKind, text: &str) -> (IdNames, ImportReport) {
        let mut n = IdNames::new(kind);
        let r = n.import(text);
        (n, r)
    }

    /// The kind of id.
    pub fn kind(&self) -> IdKind {
        self.kind
    }

    /// A counter that increases with every change. A host compares it with its saved value to decide when to save the list.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The number of entries (single ids and ranges).
    pub fn len(&self) -> usize {
        self.single.len() + self.ranges.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Imports text (see the module notes for the merge rule). The first row is a header when one of its cells is an id title
    /// (`mmsi`, `icao`, `capcode`, `talkgroup`, `radio_id`, `id` ...); the name column is then the first cell titled `name`,
    /// `alias`, `label`, `registration`, `callsign` ... Without a header the first column is the id and the second the name.
    pub fn import(&mut self, text: &str) -> ImportReport {
        let sep = Separator::detect(text);
        let mut rep = ImportReport {
            separator: Some(sep),
            ..ImportReport::default()
        };
        let mut id_col = 0usize;
        let mut name_col = Some(1usize);
        let mut titles: Vec<String> = Vec::new();
        let mut first = true;
        for rec in records(text, sep) {
            if first {
                first = false;
                let keys: Vec<String> = rec.cells.iter().map(|c| title_key(c)).collect();
                if let Some(i) = keys.iter().position(|k| ID_TITLES.contains(&k.as_str())) {
                    if self.kind.key_or_range(rec.cell(i)).is_none() {
                        rep.header = true;
                        id_col = i;
                        name_col = NAME_TITLES
                            .iter()
                            .find_map(|t| keys.iter().position(|k| k == t))
                            .or_else(|| (0..keys.len()).find(|&j| j != i));
                        titles = rec.cells.iter().map(|c| c.trim().to_lowercase()).collect();
                        continue;
                    }
                }
            }
            let title = |j: usize| -> String {
                if rep.header {
                    titles
                        .get(j)
                        .cloned()
                        .unwrap_or_else(|| format!("column{}", j + 1))
                } else if j == 0 {
                    "id".into()
                } else if j == 1 {
                    "name".into()
                } else {
                    format!("column{}", j + 1)
                }
            };
            let width = rec.cells.len().max(titles.len());
            if rep.columns.len() < width {
                rep.columns = (0..width).map(title).collect();
            }
            let Some(target) = self.kind.key_or_range(rec.cell(id_col)) else {
                reject(&mut rep, rec.line, Rejection::BadId, rec.cell(id_col));
                continue;
            };
            let mut entry = Entry {
                name: name_col.map_or(String::new(), |j| clean_name(rec.cell(j))),
                fields: Vec::new(),
            };
            for (j, c) in rec.cells.iter().enumerate() {
                if j != id_col && Some(j) != name_col {
                    entry.fields.push((title(j), clean_name(c)));
                }
            }
            if entry.is_empty() {
                reject(&mut rep, rec.line, Rejection::Empty, rec.cell(id_col));
                continue;
            }
            entry.fields.retain(|(_, v)| !v.is_empty());
            rep.read += 1;
            match self.put(target, entry) {
                Put::Added => rep.added += 1,
                Put::Changed => rep.changed += 1,
                Put::Same => {}
            }
        }
        if rep.added + rep.changed > 0 {
            self.version += 1;
        }
        rep
    }

    fn put(&mut self, target: Target, entry: Entry) -> Put {
        match target {
            Target::One(k) => match self.single.get_mut(&k) {
                Some(e) => {
                    if e.merge(entry) {
                        Put::Changed
                    } else {
                        Put::Same
                    }
                }
                None => {
                    self.single.insert(k, entry);
                    Put::Added
                }
            },
            Target::Range(a, b) => match self.ranges.iter_mut().find(|r| r.0 == a && r.1 == b) {
                Some(r) => {
                    if r.2.merge(entry) {
                        Put::Changed
                    } else {
                        Put::Same
                    }
                }
                None => {
                    self.ranges.push((a, b, entry));
                    self.ranges.sort_by_key(|r| (r.0, r.1));
                    Put::Added
                }
            },
        }
    }

    /// Sets the name of one id (or range), keeping its other columns. Returns `false` when `id` is not an id of the list's kind.
    pub fn set_name(&mut self, id: &str, name: &str) -> bool {
        let Some(t) = self.kind.key_or_range(id) else {
            return false;
        };
        let e = Entry {
            name: clean_name(name),
            fields: Vec::new(),
        };
        if !matches!(self.put(t, e), Put::Same) {
            self.version += 1;
        }
        true
    }

    /// Removes one id (or range). Returns whether it was listed.
    pub fn remove(&mut self, id: &str) -> bool {
        let removed = match self.kind.key_or_range(id) {
            Some(Target::One(k)) => self.single.remove(&k).is_some(),
            Some(Target::Range(a, b)) => {
                let n = self.ranges.len();
                self.ranges.retain(|r| !(r.0 == a && r.1 == b));
                n != self.ranges.len()
            }
            None => false,
        };
        if removed {
            self.version += 1;
        }
        removed
    }

    /// Removes every entry.
    pub fn clear(&mut self) {
        if !self.is_empty() {
            self.single.clear();
            self.ranges.clear();
            self.version += 1;
        }
    }

    /// The entry of an id: the id itself, else the narrowest range holding it.
    pub fn get(&self, id: &str) -> Option<&Entry> {
        match self.kind.key(id)? {
            Key::Num(n) => self.get_number(n),
            k => self.single.get(&k),
        }
    }

    /// The entry of a numeric id ([`IdKind::Decimal`], [`IdKind::Mmsi`], [`IdKind::Icao`]): the id itself, else the narrowest
    /// range holding it. Does not allocate.
    pub fn get_number(&self, id: u64) -> Option<&Entry> {
        if !self.kind.numeric() {
            return None;
        }
        if let Some(e) = self.single.get(&Key::Num(id)) {
            return Some(e);
        }
        self.ranges
            .iter()
            .filter(|r| (r.0..=r.1).contains(&id))
            .min_by_key(|r| r.1 - r.0)
            .map(|r| &r.2)
    }

    /// The name of an id, when it has one.
    pub fn name(&self, id: &str) -> Option<&str> {
        self.get(id)
            .map(|e| e.name.as_str())
            .filter(|n| !n.is_empty())
    }

    /// Every entry in id order (ranges after single ids), with its written id.
    pub fn iter(&self) -> impl Iterator<Item = (String, &Entry)> + '_ {
        let kind = self.kind;
        self.single
            .iter()
            .map(move |(k, e)| {
                let mut s = String::new();
                kind.write(k, &mut s);
                (s, e)
            })
            .chain(self.ranges.iter().map(move |(a, b, e)| {
                let mut s = String::new();
                kind.write_range(*a, *b, &mut s);
                (s, e)
            }))
    }

    /// Writes the list as tab-separated text with a header (`id`, `name`, then every other column title in first-seen order).
    /// Reading the export back returns the same list. Cells that a spreadsheet would read as a formula are guarded
    /// ([`export_cell`]).
    pub fn export(&self) -> String {
        let mut titles: Vec<&str> = Vec::new();
        for (_, e) in self.iter() {
            for (k, _) in &e.fields {
                if !titles.contains(&k.as_str()) {
                    titles.push(k);
                }
            }
        }
        let mut out = String::from("id\tname");
        for t in &titles {
            out.push('\t');
            out.push_str(&export_cell(t));
        }
        out.push('\n');
        for (id, e) in self.iter() {
            out.push_str(&id);
            out.push('\t');
            out.push_str(&export_cell(&e.name));
            for t in &titles {
                out.push('\t');
                out.push_str(&export_cell(e.field(t).unwrap_or("")));
            }
            out.push('\n');
        }
        out
    }
}

enum Put {
    Added,
    Changed,
    Same,
}

fn reject(rep: &mut ImportReport, line: usize, reason: Rejection, id: &str) {
    rep.rejected_count += 1;
    if rep.rejected.len() < MAX_REJECTED_LISTED {
        rep.rejected.push(Rejected {
            line,
            reason,
            id: id.chars().take(40).collect(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmsi_list_with_header_and_spreadsheet_damage() {
        let text = "# ships\nMMSI\tShip name\tCall sign\tType\n2320064\tSolent Coastguard\t\tCoast\n'232123456\tMV Example\tMABC2\tCargo\n\
                    123\tShort\t\t\n232000001\t\t\t\n";
        let (n, r) = IdNames::from_text(IdKind::Mmsi, text);
        assert!(r.header);
        assert_eq!(r.separator, Some(Separator::Tab));
        assert_eq!(n.name("002320064"), Some("Solent Coastguard"));
        assert_eq!(n.name("2320064"), Some("Solent Coastguard"));
        assert_eq!(
            n.get("232123456").unwrap().field("Call sign"),
            Some("MABC2")
        );
        assert_eq!(r.read, 2);
        assert_eq!(r.rejected_count, 2);
        assert_eq!(
            r.rejected[0],
            Rejected {
                line: 5,
                reason: Rejection::BadId,
                id: "123".into()
            }
        );
        assert_eq!(r.rejected[1].reason, Rejection::Empty);
        assert_eq!(r.rejected[1].line, 6);
    }

    #[test]
    fn merge_rule_empty_never_erases() {
        let mut n = IdNames::new(IdKind::Mmsi);
        n.import("mmsi;name;callsign\n232123456;Ship;CALL\n");
        let v = n.version();
        let r = n.import("mmsi;name;callsign\n232123456;;\n");
        assert_eq!(r.rejected_count, 1, "a row with nothing to say is refused");
        assert_eq!(n.name("232123456"), Some("Ship"));
        let r = n.import("mmsi;name;callsign\n232123456;Renamed;\n");
        assert_eq!((r.added, r.changed), (0, 1));
        assert_eq!(n.get("232123456").unwrap().field("callsign"), Some("CALL"));
        assert!(n.version() > v);
        let v = n.version();
        let r = n.import("mmsi;name\n232123456;Renamed\n");
        assert_eq!((r.added, r.changed), (0, 0));
        assert_eq!(n.version(), v, "no change, no new version");
    }

    #[test]
    fn capcode_ranges_take_the_narrowest() {
        let (n, r) = IdNames::from_text(
            IdKind::Decimal,
            "capcode,name\n2000-2099,Fire\n2050-2010,Station 5\n2042,Pump 2\n",
        );
        assert_eq!(r.added, 3);
        assert_eq!(n.name("2042"), Some("Pump 2"));
        assert_eq!(n.name("2011"), Some("Station 5"));
        assert_eq!(n.name("2099"), Some("Fire"));
        assert_eq!(n.name("2100"), None);
        assert_eq!(n.get_number(2030).unwrap().name, "Station 5");
    }

    #[test]
    fn talkgroups_as_equals_lines_without_header() {
        let (n, r) = IdNames::from_text(IdKind::Decimal, "91=Worldwide\n92 = Europe\nTG x=bad\n");
        assert_eq!(r.separator, Some(Separator::Equals));
        assert!(!r.header);
        assert_eq!(n.name("91"), Some("Worldwide"));
        assert_eq!(n.name("92"), Some("Europe"));
        assert_eq!(r.rejected_count, 1);
    }

    #[test]
    fn radio_id_csv_keeps_other_columns() {
        let text = "RADIO_ID,CALLSIGN,FIRST_NAME,LAST_NAME,CITY\n3120123,W1ABC,Joe,Smith,Boston\n";
        let (n, _) = IdNames::from_text(IdKind::Decimal, text);
        let e = n.get("3120123").unwrap();
        assert_eq!(e.name, "W1ABC");
        assert_eq!(e.field("first_name"), Some("Joe"));
        assert_eq!(e.field("city"), Some("Boston"));
    }

    #[test]
    fn aircraft_semicolon_database_and_selcal_codes() {
        let text = "icao24;registration;typecode;operator\n4ca123;EI-ABC;B738;Ryanair\nA835AF;N628TS;GLF6;\nZZZ;bad;;\n";
        let (n, r) = IdNames::from_text(IdKind::Icao, text);
        assert_eq!(n.name("4CA123"), Some("EI-ABC"));
        assert_eq!(n.get("a835af").unwrap().field("typecode"), Some("GLF6"));
        assert_eq!(r.rejected_count, 1);
        let (s, _) = IdNames::from_text(IdKind::Selcal, "AMCP\tG-ABCD\nam-cq\tF-XYZW\n");
        assert_eq!(s.name("AM-CP"), Some("G-ABCD"));
        assert_eq!(s.name("AMCQ"), Some("F-XYZW"));
    }

    #[test]
    fn export_round_trip() {
        let text = "id\tname\tnote\n91\tWorldwide\t=SUM(A1)\n2000-2099\tFire\t\n";
        let (n, _) = IdNames::from_text(IdKind::Decimal, text);
        let out = n.export();
        assert!(out.contains("'=SUM(A1)"));
        let (m, r) = IdNames::from_text(IdKind::Decimal, &out);
        assert_eq!(r.rejected_count, 0);
        assert_eq!(m.len(), n.len());
        assert_eq!(m.name("2050"), Some("Fire"));
    }

    #[test]
    fn set_remove_clear() {
        let mut n = IdNames::new(IdKind::Text);
        assert!(n.set_name("  Station   One ", "A"));
        assert_eq!(n.name("station one"), Some("A"));
        assert!(n.remove("STATION ONE"));
        assert!(!n.remove("STATION ONE"));
        n.set_name("x", "y");
        n.clear();
        assert!(n.is_empty());
        assert!(!IdNames::new(IdKind::Mmsi).set_name("12", "too short"));
    }

    #[test]
    fn long_names_are_cut() {
        let long = "x".repeat(500);
        let (n, _) = IdNames::from_text(IdKind::Decimal, &format!("1,{long}\n"));
        assert_eq!(n.name("1").unwrap().chars().count(), MAX_NAME_CHARS);
    }
}
