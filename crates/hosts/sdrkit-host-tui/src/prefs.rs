//! The settings file of the application: what it keeps between runs, one `key=value` a line. The engine does not
//! read it.

use std::path::{Path, PathBuf};

/// The key of the chosen region.
const REGION: &str = "region";

/// The settings of the application and the file that holds them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    /// The file, or `None` to keep nothing.
    file: Option<PathBuf>,
    /// The country that was chosen last (ISO 3166 code).
    pub region: Option<String>,
}

impl Prefs {
    /// The settings in `file`. A file that does not exist or cannot be read gives none, and a line with another key is
    /// skipped. `None` keeps nothing.
    pub fn load(file: Option<&Path>) -> Prefs {
        let text = file.and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
        let value = |key: &str| {
            text.lines()
                .filter_map(|l| l.split_once('='))
                .find(|(k, _)| k.trim() == key)
                .map(|(_, v)| v.trim().to_owned())
                .filter(|v| !v.is_empty())
        };
        Prefs {
            file: file.map(Path::to_path_buf),
            region: value(REGION),
        }
    }

    /// Writes the settings to the file, and makes its folder if it does not exist. Does nothing without a file.
    ///
    /// # Errors
    ///
    /// The error of the file system.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut text = String::new();
        if let Some(r) = &self.region {
            text.push_str(&format!("{REGION}={r}\n"));
        }
        std::fs::write(file, text)
    }
}
