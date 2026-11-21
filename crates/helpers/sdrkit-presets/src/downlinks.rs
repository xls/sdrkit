//! Satellite downlinks and the data files that stacks read through their settings.
//!
//! A downlink list is a CSV file under `links/` with the columns `band,submode,name,centre_mhz,width_mhz,symbol_rate,
//! spacecraft,note`, where `band` is the stack id. A data file names the stacks that read it in a `# stacks:` line at its
//! top.

use crate::data::{read_text, Data, Table};
use crate::error::DataError;
use std::path::Path;

/// One row of a downlink list: the downlink of one satellite in one submode of a stack.
#[derive(Clone, Debug, PartialEq)]
pub struct Downlink {
    /// The stack id.
    pub stack: String,
    /// The submode of the stack.
    pub submode: String,
    /// The satellite or service name.
    pub name: String,
    /// The downlink centre, Hz. With a down-converter the radio tunes the converted frequency.
    pub frequency_hz: f64,
    /// The width of the signal, Hz, if the row has one.
    pub width_hz: Option<f64>,
    /// The symbol rate, symbols a second, if the row has one.
    pub symbol_rate: Option<f64>,
    /// The spacecraft id of the frame headers, if the row has one.
    pub spacecraft: Option<u32>,
    /// The note of the row.
    pub note: String,
}

/// The columns of a downlink list.
const COLUMNS: [&str; 8] = [
    "band",
    "submode",
    "name",
    "centre_mhz",
    "width_mhz",
    "symbol_rate",
    "spacecraft",
    "note",
];

/// The `.csv` files of `dir`, in name order. An absent folder has none.
fn csv_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "csv"))
        .collect();
    files.sort();
    files
}

/// The stack ids of the `# stacks:` line at the top of `text`.
fn stacks_line(text: &str) -> Vec<&str> {
    text.lines()
        .take_while(|l| l.starts_with('#'))
        .find_map(|l| l.trim_start_matches('#').trim().strip_prefix("stacks:"))
        .map(|s| s.split_whitespace().collect())
        .unwrap_or_default()
}

impl Data {
    /// The downlinks of `stack` from the downlink lists of `links/`, in the order of the files. A file under `links/`
    /// without the columns of a downlink list is another kind of list and is passed over.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`], and [`DataError::Format`] for a row whose frequency is not a number.
    pub fn downlinks(&self, stack: &str) -> Result<Vec<Downlink>, DataError> {
        let mut out = Vec::new();
        for path in csv_files(&self.folder().join("links")) {
            let t = match Table::read(path, &COLUMNS) {
                Ok(t) => t,
                Err(DataError::Format { .. }) => continue,
                Err(e) => return Err(e),
            };
            for (line, r) in &t.rows {
                if t.text(r, "band") != stack {
                    continue;
                }
                out.push(Downlink {
                    stack: stack.to_owned(),
                    submode: t.text(r, "submode").to_owned(),
                    name: t.text(r, "name").to_owned(),
                    frequency_hz: t.required(*line, r, "centre_mhz")? * 1e6,
                    width_hz: t.number(*line, r, "width_mhz")?.map(|w| w * 1e6),
                    symbol_rate: t.number(*line, r, "symbol_rate")?,
                    spacecraft: t
                        .number(*line, r, "spacecraft")?
                        .and_then(|v| u32::try_from(v as i64).ok()),
                    note: t.text(r, "note").to_owned(),
                });
            }
        }
        Ok(out)
    }

    /// The data files that `stack` reads through its settings: the CSV files of the folder, down to one folder deep,
    /// whose `# stacks:` line names it. Each is the path from the folder (`links/xband_downlinks.csv`), which is the path
    /// in the host's storage that the stack's settings name. A host copies these files into its storage.
    ///
    /// # Errors
    ///
    /// [`DataError::Read`] for a file that cannot be read.
    pub fn stack_files(&self, stack: &str) -> Result<Vec<String>, DataError> {
        let mut dirs = vec![self.folder().to_owned()];
        if let Ok(entries) = std::fs::read_dir(self.folder()) {
            let mut sub: Vec<_> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            sub.sort();
            dirs.extend(sub);
        }
        let mut out = Vec::new();
        for dir in dirs {
            for path in csv_files(&dir) {
                let text = read_text(&path)?;
                if !stacks_line(&text).contains(&stack) {
                    continue;
                }
                if let Ok(rel) = path.strip_prefix(self.folder()) {
                    let parts: Vec<_> = rel.iter().map(|p| p.to_string_lossy()).collect();
                    out.push(parts.join("/"));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::stacks_line;

    #[test]
    fn the_stacks_line_is_read_from_the_top_only() {
        assert_eq!(stacks_line("# id: x\n# stacks: rdas eodb\na,b\n"), ["rdas", "eodb"]);
        assert!(stacks_line("a,b\n# stacks: rdas\n").is_empty());
    }
}
