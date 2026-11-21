//! The reader of a file of entries that the host supplies: the frequencies that a rule or a published convention sets
//! for a service. The columns are `slug,name,mhz,upper_mhz,bandwidth_hz,stack,submode,category,tags,scope,notes,source`.
//! `upper_mhz` and `bandwidth_hz` are 0 for none. `tags` and `scope` hold words apart by spaces.

use crate::entry::{Category, Entry};
use crate::error::{LineProblem, Problem};
use sdrkit_util_text::csv;

/// The entries of the file `text`, each with the id `b:<slug>`. A line that starts with `#` is a comment, and the first
/// other line names the columns.
///
/// # Errors
///
/// The first line that is not a row of twelve fields with a known category and numbers where numbers belong.
pub fn supplied_entries(text: &str) -> Result<Vec<Entry>, LineProblem> {
    let mut out = Vec::new();
    let mut header = true;
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if std::mem::take(&mut header) {
            continue;
        }
        let bad = |kind| LineProblem { line: i + 1, kind };
        let f = csv::split_line(line, ',');
        let [slug, name, mhz, end, bw, stack, sub, cat, tags, scope, notes, source] = &f[..] else {
            return Err(bad(Problem::NoFrequency));
        };
        let number = |t: &String| t.trim().parse::<f64>().map_err(|_| bad(Problem::BadFrequency));
        let (mhz, end, bw) = (number(mhz)?, number(end)?, number(bw)?);
        let cat = Category::from_id(cat).ok_or(bad(Problem::NotAnEntry))?;
        // Every supplied frequency is a whole number of hertz.
        let mut e = Entry::new(format!("b:{slug}"), name.as_str(), (mhz * 1e6).round(), cat);
        e.end_hz = (end > 0.0).then_some((end * 1e6).round());
        e.bandwidth_hz = (bw > 0.0).then_some(bw);
        e.stack.clone_from(stack);
        e.submode.clone_from(sub);
        e.tags = tags.split_whitespace().map(String::from).collect();
        e.scope = scope.split_whitespace().map(String::from).collect();
        e.notes.clone_from(notes);
        e.source.clone_from(source);
        out.push(e);
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The entries of the repository's `data/freqdb/builtin.csv`.
    pub(crate) fn supplied() -> Vec<Entry> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../data/freqdb/builtin.csv");
        supplied_entries(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn rows_are_sound() {
        let all = supplied();
        assert_eq!(all.len(), 183);
        let mut ids: Vec<&str> = all.iter().map(|e| e.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        for e in &all {
            assert!(e.hz > 0.0 && e.hz < 100e9, "{}", e.id);
            if let Some(end) = e.end_hz {
                assert!(end > e.hz, "{}", e.id);
            }
            assert!(!e.source.is_empty(), "{}", e.id);
            for c in &e.scope {
                assert!(
                    ["itu1", "itu2", "itu3"].contains(&c.as_str())
                        || (c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase())),
                    "{} {}",
                    e.id,
                    c
                );
            }
            if e.stack == "general" {
                assert!(
                    ["usb", "lsb", "am", "nfm", "wfm", "cw"].contains(&e.submode.as_str()),
                    "{}",
                    e.id
                );
            }
        }
        let iss = all.iter().find(|e| e.id == "b:iss-voice").unwrap();
        assert_eq!(iss.hz, 145.8e6);
        assert!(supplied_entries("h\na,b").is_err());
    }
}
