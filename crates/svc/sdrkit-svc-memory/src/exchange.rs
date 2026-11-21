//! Import and export of memories: JSON (the memory file, or a bare array), the store's own CSV, a plain `frequency,name` list, and
//! CSV exports of frequency managers whose header names the columns (`Frequency`/`Freq`/`MHz`/`kHz`, `Name`/`Description`/`Tag`/
//! `Label`, `Mode`/`Modulation`/`Demodulator`, `Bandwidth`/`Filter`, `Group`/`Folder`/`Category`/`Bank`, `Notes`/`Comment`).
//! Duplicates (the same stack and frequency, already in the store or earlier in the file) are skipped.

use crate::error::{LineProblem, MemoryError, Problem};
use crate::freq::{self, UnitHint};
use crate::memory::Memory;
use crate::store::MemoryStore;
use sdrkit_core::OwnerSetting;
use sdrkit_util_text::csv;
use serde::Serialize;

sdrkit_core::choice_enum! {
    /// A file format of memories.
    pub enum Format {
        /// Decided from the content: JSON if it starts with `{` or `[`, else CSV with a header, else a plain list.
        Auto = ("auto", "Automatic"),
        /// JSON.
        Json = ("json", "JSON"),
        /// CSV with a header row: the columns of the store, or of a frequency manager.
        Csv = ("csv", "CSV"),
        /// One `frequency,name` per line with no header. The delimiter is `,`, `;` or a tab.
        Plain = ("plain", "Plain list"),
    }
}

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    /// Memories added.
    pub added: usize,
    /// Rows skipped because the channel was already there.
    pub duplicates: usize,
    /// The rows that could not be read.
    pub problems: Vec<LineProblem>,
    /// Rows not added because the store was full.
    pub over_cap: usize,
}

/// The store's own CSV columns, in order.
pub const CSV_COLUMNS: [&str; 20] = [
    "name",
    "frequency_hz",
    "mode",
    "submode",
    "bandwidth_hz",
    "rate_hz",
    "group",
    "tags",
    "notes",
    "color",
    "starred",
    "scan",
    "lockout",
    "priority",
    "dwell_s",
    "tone",
    "hits",
    "last_hit_s",
    "gains",
    "settings",
];

/// Reads memories from text. Rows are validated; ids, order and times are given by the store.
pub(crate) fn parse(text: &str, format: Format) -> (Vec<Memory>, Vec<LineProblem>) {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let format = match format {
        Format::Auto if trimmed.starts_with('{') || trimmed.starts_with('[') => Format::Json,
        Format::Auto => {
            let d = csv::sniff_delimiter(trimmed);
            match csv::read(trimmed, d).first() {
                Some((_, first)) if header_map(first).is_some() => Format::Csv,
                _ => Format::Plain,
            }
        }
        f => f,
    };
    match format {
        Format::Json => parse_json(trimmed),
        Format::Csv => parse_csv(trimmed),
        _ => parse_plain(trimmed),
    }
}

/// Imports text into a store, skipping duplicates (a memory of the same stack within `tolerance_hz`). `now_s` becomes the
/// creation time of the new memories.
pub(crate) fn import(
    store: &mut MemoryStore,
    text: &str,
    format: Format,
    now_s: f64,
    tolerance_hz: f64,
) -> ImportReport {
    let (list, problems) = parse(text, format);
    let mut r = ImportReport {
        problems,
        ..ImportReport::default()
    };
    for mut m in list {
        if store.find_same(m.freq_hz, &m.stack, tolerance_hz).is_some() {
            r.duplicates += 1;
            continue;
        }
        m.id = 0;
        m.created_s = if m.created_s > 0.0 {
            m.created_s
        } else {
            now_s
        };
        match store.insert(m, now_s) {
            Ok(_) => r.added += 1,
            Err(MemoryError::Invalid(kind)) => r.problems.push(LineProblem { line: 0, kind }),
            Err(_) => r.over_cap += 1,
        }
    }
    r
}

fn parse_json(text: &str) -> (Vec<Memory>, Vec<LineProblem>) {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Any {
        File { memories: Vec<serde_json::Value> },
        List(Vec<serde_json::Value>),
    }
    let values = match serde_json::from_str::<Any>(text) {
        Ok(Any::File { memories }) | Ok(Any::List(memories)) => memories,
        Err(e) => {
            let line = e.line().max(1);
            return (
                Vec::new(),
                vec![LineProblem {
                    line,
                    kind: Problem::NotJson,
                }],
            );
        }
    };
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, v) in values.into_iter().enumerate() {
        match serde_json::from_value::<Memory>(v) {
            Ok(m) => match m.validate() {
                Ok(()) => out.push(m),
                Err(e) => errors.push(LineProblem {
                    line: i + 1,
                    kind: problem_of(&e),
                }),
            },
            Err(_) => errors.push(LineProblem {
                line: i + 1,
                kind: Problem::NotAMemory,
            }),
        }
    }
    (out, errors)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
    Name,
    Freq(UnitHint),
    FreqHz,
    Mode,
    Submode,
    Bandwidth(UnitHint),
    Rate,
    Group,
    Tags,
    Notes,
    Color,
    Starred,
    Scan,
    Lockout,
    Priority,
    Dwell,
    Tone,
    Hits,
    LastHit,
    Gains,
    Settings,
}

fn column(header: &str) -> Option<Col> {
    let h = header.trim().to_ascii_lowercase();
    let unit = UnitHint::from_header(&h);
    let base: String = h
        .split(['(', '[', ' '])
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    Some(match base.as_str() {
        "frequency_hz" => Col::FreqHz,
        "frequency" | "freq" | "freq." | "mhz" | "khz" | "hz" | "rx" | "rx_freq" | "receive" => {
            Col::Freq(if base == "mhz" {
                UnitHint::MHz
            } else if base == "khz" {
                UnitHint::KHz
            } else if base == "hz" {
                UnitHint::Hz
            } else {
                unit
            })
        }
        "name" | "description" | "desc" | "tag" | "label" | "station" | "alpha" | "title" => {
            Col::Name
        }
        "mode" | "modulation" | "demodulator" | "demod" => Col::Mode,
        "submode" => Col::Submode,
        "bandwidth_hz" => Col::Bandwidth(UnitHint::Hz),
        "bandwidth" | "bw" | "filter" => Col::Bandwidth(match unit {
            UnitHint::Guess => UnitHint::Hz,
            u => u,
        }),
        "rate_hz" | "samplerate" | "sample_rate" => Col::Rate,
        "group" | "folder" | "category" | "bank" | "service" => Col::Group,
        "tags" => Col::Tags,
        "notes" | "note" | "comment" | "comments" | "remarks" => Col::Notes,
        "color" | "colour" => Col::Color,
        "starred" | "favorite" | "favourite" | "star" => Col::Starred,
        "scan" => Col::Scan,
        "lockout" | "skip" => Col::Lockout,
        "priority" => Col::Priority,
        "dwell_s" | "dwell" => Col::Dwell,
        "tone" | "ctcss" | "dcs" => Col::Tone,
        "hits" => Col::Hits,
        "last_hit_s" => Col::LastHit,
        "gains" => Col::Gains,
        "settings" => Col::Settings,
        _ => return None,
    })
}

/// A header row maps when it names a frequency column.
fn header_map(row: &[String]) -> Option<Vec<Option<Col>>> {
    let cols: Vec<Option<Col>> = row.iter().map(|h| column(h)).collect();
    cols.iter()
        .any(|c| matches!(c, Some(Col::Freq(_)) | Some(Col::FreqHz)))
        .then_some(cols)
}

/// The modulation names of other frequency lists, as a stack id and a demodulator.
fn mode_of(text: &str) -> (String, String) {
    let t = text.trim().to_ascii_lowercase();
    let general = |d: &str| ("general".to_string(), d.to_string());
    match t.as_str() {
        "" => (String::new(), String::new()),
        "nfm" | "fm-n" | "fmn" | "nbfm" | "fm narrow" => general("nfm"),
        "am" | "am-n" | "amn" => general("am"),
        "usb" => general("usb"),
        "lsb" => general("lsb"),
        "cw" | "cw-u" | "cw-l" | "cwu" | "cwl" => general("cw"),
        "wfm" | "wbfm" | "fm-w" | "fmw" | "bfm" | "broadcast fm" => ("fm".into(), String::new()),
        "fm" => general("nfm"),
        other => (other.to_string(), String::new()),
    }
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on" | "x" | "*"
    )
}

/// Reads a cell of stored settings: empty, or a JSON list of settings with their owners and typed values.
pub(crate) fn settings_cell(s: &str) -> Result<Vec<OwnerSetting>, Problem> {
    if s.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(s).map_err(|_| Problem::NotSettings)
}

/// Writes a cell of stored settings in the form that [`settings_cell`] reads.
pub(crate) fn settings_text(list: &[OwnerSetting]) -> String {
    if list.is_empty() {
        return String::new();
    }
    serde_json::to_string(list).unwrap_or_default()
}

fn problem_of(e: &MemoryError) -> Problem {
    match e {
        MemoryError::Invalid(p) => *p,
        _ => Problem::Full,
    }
}

fn opt_num(s: &str) -> Result<Option<f64>, Problem> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<f64>().map(Some).map_err(|_| Problem::NotANumber)
}

fn parse_csv(text: &str) -> (Vec<Memory>, Vec<LineProblem>) {
    let d = csv::sniff_delimiter(text);
    let rows = csv::read(text, d);
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let Some(((_, header), body)) = rows.split_first() else {
        return (out, errors);
    };
    let Some(cols) = header_map(header) else {
        errors.push(LineProblem {
            line: 1,
            kind: Problem::NoFrequencyColumn,
        });
        return (out, errors);
    };
    // The layout of the store keeps stack ids as they are. Other layouts name modulations.
    let own = cols.contains(&Some(Col::FreqHz));
    for (line, row) in body {
        match csv_row(&cols, row, own) {
            Ok(m) => match m.validate() {
                Ok(()) => out.push(m),
                Err(e) => errors.push(LineProblem {
                    line: *line,
                    kind: problem_of(&e),
                }),
            },
            Err(kind) => errors.push(LineProblem { line: *line, kind }),
        }
    }
    (out, errors)
}

fn csv_row(cols: &[Option<Col>], row: &[String], own: bool) -> Result<Memory, Problem> {
    let mut m = Memory::default();
    let mut have_freq = false;
    let mut mode_text = None;
    for (c, v) in cols.iter().zip(row) {
        let Some(c) = c else { continue };
        let v = v.trim();
        match *c {
            Col::Name => m.name = v.to_string(),
            Col::FreqHz => {
                m.freq_hz = v.parse().map_err(|_| Problem::NotAFrequency)?;
                have_freq = true;
            }
            Col::Freq(hint) => {
                m.freq_hz = freq::parse_hz(v, hint).ok_or(Problem::NotAFrequency)?;
                have_freq = true;
            }
            Col::Mode => mode_text = Some(v.to_string()),
            Col::Submode => m.submode = v.to_string(),
            Col::Bandwidth(hint) => {
                if !v.is_empty() {
                    m.bandwidth_hz = Some(freq::parse_hz(v, hint).ok_or(Problem::NotABandwidth)?)
                }
            }
            Col::Rate => m.rate_hz = opt_num(v)?,
            Col::Group => m.group = v.to_string(),
            Col::Tags => {
                m.tags = v
                    .split([';', ','])
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(String::from)
                    .collect()
            }
            Col::Notes => m.notes = v.to_string(),
            Col::Color => m.color = v.to_string(),
            Col::Starred => m.starred = truthy(v),
            Col::Scan => m.scan = v.is_empty() || truthy(v),
            Col::Lockout => m.lockout = truthy(v),
            Col::Priority => m.priority = truthy(v),
            Col::Dwell => m.dwell_s = opt_num(v)?,
            Col::Tone => m.tone = v.to_string(),
            Col::Hits => m.hits = opt_num(v)?.unwrap_or(0.0).max(0.0) as u64,
            Col::LastHit => m.last_hit_s = opt_num(v)?,
            Col::Gains => m.gains = settings_cell(v)?,
            Col::Settings => m.settings = settings_cell(v)?,
        }
    }
    if !have_freq {
        return Err(Problem::NoFrequency);
    }
    if let Some(t) = mode_text {
        let (stack, demod) = if own { (t, String::new()) } else { mode_of(&t) };
        m.stack = stack;
        if m.submode.is_empty() {
            m.submode = demod;
        }
    }
    Ok(m)
}

fn parse_plain(text: &str) -> (Vec<Memory>, Vec<LineProblem>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        // Each line may use its own delimiter.
        let d = csv::sniff_delimiter(t);
        let row = csv::read(t, d)
            .into_iter()
            .next()
            .map(|r| r.1)
            .unwrap_or_default();
        let f = row.first().map(|s| s.trim()).unwrap_or("");
        match freq::parse_hz(f, UnitHint::Guess) {
            Some(hz) => {
                let name = row
                    .get(1..)
                    .map(|r| r.join(&d.to_string()))
                    .unwrap_or_default();
                out.push(Memory::new(name.trim(), hz, ""));
            }
            None => errors.push(LineProblem {
                line: i + 1,
                kind: Problem::NotAFrequency,
            }),
        }
    }
    (out, errors)
}

/// Exports memories as JSON (the memory file layout).
pub fn export_json(memories: &[&Memory]) -> String {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        memories: &'a [&'a Memory],
    }
    serde_json::to_string_pretty(&Out {
        version: crate::files::FILE_VERSION,
        memories,
    })
    .unwrap_or_default()
}

/// Exports memories as CSV with [`CSV_COLUMNS`]. Tags are joined with `;`. Gains and settings are JSON lists of settings
/// with their owners and typed values.
pub fn export_csv(memories: &[&Memory]) -> String {
    let mut out = csv::record(&CSV_COLUMNS, ',');
    out.push('\n');
    let num = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
    let b = |v: bool| if v { "1" } else { "0" }.to_string();
    for m in memories {
        let row = [
            m.name.clone(),
            m.freq_hz.to_string(),
            m.stack.clone(),
            m.submode.clone(),
            num(m.bandwidth_hz),
            num(m.rate_hz),
            m.group.clone(),
            m.tags.join(";"),
            m.notes.clone(),
            m.color.clone(),
            b(m.starred),
            b(m.scan),
            b(m.lockout),
            b(m.priority),
            num(m.dwell_s),
            m.tone.clone(),
            m.hits.to_string(),
            num(m.last_hit_s),
            settings_text(&m.gains),
            settings_text(&m.settings),
        ];
        out.push_str(&csv::record(&row, ','));
        out.push('\n');
    }
    out
}

/// Exports a plain `frequency,name` list, the frequency in Hz.
pub fn export_plain(memories: &[&Memory]) -> String {
    let mut out = String::new();
    for m in memories {
        out.push_str(&csv::record(
            &[format!("{}", m.freq_hz), m.name.clone()],
            ',',
        ));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::{OwnerKey, Value};

    fn sample() -> MemoryStore {
        let mut s = MemoryStore::new();
        let general = OwnerKey::of("stack.general");
        let mut a = Memory::new("Tower, main", 118.1e6, "general")
            .with_setting(general.clone(), "demod", Value::Choice(2))
            .with_setting(general, "width", Value::F64(8330.0));
        a.group = "Airport".into();
        a.tags = vec!["atc".into(), "local".into()];
        a.notes = "say \"hi\"".into();
        a.color = "#00ff00".into();
        a.starred = true;
        a.dwell_s = Some(2.5);
        a.gains.push(OwnerSetting {
            owner: OwnerKey::of("radio"),
            key: "lna".into(),
            value: Value::F64(30.0),
        });
        a.tone = "CTCSS 88.5 Hz".into();
        s.insert(a, 10.0).unwrap();
        s.insert(Memory::new("Radio", 98.8e6, "fm"), 20.0).unwrap();
        s
    }

    fn all(s: &MemoryStore) -> Vec<&Memory> {
        s.memories().iter().collect()
    }

    #[test]
    fn csv_round_trip() {
        let s = sample();
        let text = export_csv(&all(&s));
        let mut t = MemoryStore::new();
        let r = import(&mut t, &text, Format::Auto, 99.0, 1.0);
        assert_eq!((r.added, r.duplicates, r.problems.len()), (2, 0, 0));
        let a = &t.memories()[0];
        let o = &s.memories()[0];
        assert_eq!(a.name, o.name);
        assert_eq!(a.freq_hz, o.freq_hz);
        assert_eq!(a.settings, o.settings);
        assert_eq!(a.gains, o.gains);
        assert_eq!(a.tags, o.tags);
        assert_eq!(a.notes, o.notes);
        assert_eq!((a.starred, a.dwell_s), (true, Some(2.5)));
        assert_eq!(a.tone, o.tone);
        assert_eq!(t.memories()[1].stack, "fm", "mode ids stay as they are");
        // Importing again only finds duplicates.
        let r = import(&mut t, &text, Format::Csv, 99.0, 1.0);
        assert_eq!((r.added, r.duplicates), (0, 2));
    }

    #[test]
    fn json_round_trip() {
        let s = sample();
        let text = export_json(&all(&s));
        let mut t = MemoryStore::new();
        let r = import(&mut t, &text, Format::Auto, 0.0, 1.0);
        assert_eq!(r.added, 2);
        assert_eq!(t.memories()[0].settings, s.memories()[0].settings);
        assert_eq!(t.memories()[0].created_s, 10.0, "times survive");
        let r = import(
            &mut MemoryStore::new(),
            "[{\"freq_hz\":-1}]",
            Format::Json,
            0.0,
            1.0,
        );
        assert_eq!(r.problems.len(), 1);
    }

    #[test]
    fn plain_list() {
        let text = "145.500,Calling channel\n446.00625;PMR 1\n7100k,Forty, metres\nnonsense,x\n";
        let (list, errors) = parse(text, Format::Auto);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].freq_hz, 145.5e6);
        assert_eq!(list[1].freq_hz, 446.00625e6);
        assert_eq!(list[1].name, "PMR 1");
        assert_eq!(list[2].name, "Forty, metres");
        assert_eq!(errors.len(), 1);
        let back = export_plain(&list.iter().collect::<Vec<_>>());
        assert!(back.starts_with("145500000,Calling channel\n"));
    }

    #[test]
    fn frequency_manager_exports_by_header() {
        let text = "Frequency (kHz);Name;Modulation;Bandwidth;Folder;Comment\n\
                    7100;Forty;LSB;2700;HF;\n\
                    118100;Tower;AM;8330;Air;busy\n\
                    98800;Radio;WFM;200000;Broadcast;\n";
        let (list, errors) = parse(text, Format::Auto);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(list[0].freq_hz, 7.1e6);
        assert_eq!(list[0].stack, "general");
        assert_eq!(list[0].submode, "lsb");
        assert_eq!(list[0].bandwidth_hz, Some(2700.0));
        assert_eq!(list[1].group, "Air");
        assert_eq!(list[1].notes, "busy");
        assert_eq!(list[2].stack, "fm");
        let text = "MHz,Description,Mode\n145.5,Calling,NFM\n";
        let (list, _) = parse(text, Format::Auto);
        assert_eq!(list[0].freq_hz, 145.5e6);
        assert_eq!(list[0].submode, "nfm");
    }

    #[test]
    fn duplicates_inside_one_file_are_skipped() {
        let mut s = MemoryStore::new();
        let r = import(&mut s, "100.0,A\n100.0,B\n", Format::Plain, 0.0, 1.0);
        assert_eq!((r.added, r.duplicates), (1, 1));
    }

    #[test]
    fn a_full_store_counts_what_did_not_fit() {
        let mut s = MemoryStore::with_cap(1);
        let r = import(&mut s, "100.0,A\n101.0,B\n", Format::Plain, 0.0, 1.0);
        assert_eq!((r.added, r.over_cap), (1, 1));
    }
}
