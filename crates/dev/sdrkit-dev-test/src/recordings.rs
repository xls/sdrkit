//! Where recordings are stored. Recordings (`.sigmf-data` and the other sample and picture files) are never kept in git: they
//! are in the folder named by `SDRKIT_RECORDINGS`, else `../sdrkit-recordings` beside the workspace, laid out as
//! `<family>/<name>`. The repository keeps each one's `.sigmf-meta`, its expected results and its entry in
//! `tests/vectors/manifest.json`.
//!
//! A test that reads a recording calls [`recording`] (or [`read_recording`]). When it is missing, the test prints why and
//! returns early: a missing recording skips a test, it never fails one.
//!
//! ```
//! # fn _example() {
//! let Some(rec) = sdrkit_dev_test::read_recording("dvbt/dvbt_7mhz_2k_16qam_8msps") else {
//!     return; // skipped, with the reason printed
//! };
//! assert!(rec.rate_hz > 0.0);
//! # }
//! ```

use crate::sigmf;
use std::path::PathBuf;

/// The environment variable naming the recordings folder.
pub const RECORDINGS_ENV: &str = "SDRKIT_RECORDINGS";

/// The workspace root (the folder holding `tests/vectors`).
fn workspace_root() -> PathBuf {
    let v = sigmf::vectors_dir();
    v.parent()
        .and_then(|t| t.parent())
        .map(PathBuf::from)
        .unwrap_or(v)
}

/// The recordings folder: `SDRKIT_RECORDINGS` (a relative path is taken from the workspace root), else `../sdrkit-recordings`
/// beside the workspace. It may not exist.
pub fn recordings_dir() -> PathBuf {
    let root = workspace_root();
    match std::env::var_os(RECORDINGS_ENV).filter(|v| !v.is_empty()) {
        Some(v) => root.join(PathBuf::from(v)),
        None => root.join("..").join("sdrkit-recordings"),
    }
}

/// The file of a recording, by `<family>/<name>`: a name without an extension is a SigMF recording (`<name>.sigmf-data`), one
/// with an extension is that file (`dvbt/mux_two_services.ts`, `satimg/goes_file0.lrit`). `None` when it is missing, after
/// printing which recording and how to get it.
pub fn recording(name: &str) -> Option<PathBuf> {
    let rel = if PathBuf::from(name).extension().is_some() {
        PathBuf::from(name)
    } else {
        PathBuf::from(format!("{name}.sigmf-data"))
    };
    let dir = recordings_dir();
    let p = dir.join(&rel);
    if p.is_file() {
        return Some(p);
    }
    eprintln!(
        "skipped: the recording {} is not in {} (set {RECORDINGS_ENV} to the recordings folder, or run \
         `sdrkit vectors generate {name}`; `sdrkit vectors list` shows what is missing)",
        rel.display(),
        dir.display()
    );
    None
}

/// A SigMF recording by `<family>/<name>`: its `.sigmf-meta` from `tests/vectors`, its samples from the recordings folder.
/// `None` (with the reason printed) when the samples are missing; a recording that is there but cannot be read panics with the
/// reason, since that is a broken test, not a missing file.
pub fn read_recording(name: &str) -> Option<sigmf::Recording> {
    let data = recording(name)?;
    let meta = sigmf::vectors_dir().join(format!("{name}.sigmf-meta"));
    Some(sigmf::read_pair(&meta, &data).unwrap_or_else(|e| panic!("{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_recording_is_none_not_a_failure() {
        assert!(recording("nothing/no_such_recording").is_none());
        assert!(read_recording("nothing/no_such_recording").is_none());
        assert!(recording("nothing/no_such_picture.png").is_none());
    }

    #[test]
    fn the_dab_ci16_le_recording_reads_whole() {
        let Some(rec) = read_recording("real/dab/dab_9a_sydney") else {
            return;
        };
        // Its .sigmf-meta: ci16_le, 2.1 MHz, 202.928 MHz, core:sample_count 8692752.
        assert_eq!((rec.rate_hz, rec.centre_hz), (2.1e6, 202.928e6));
        assert_eq!(rec.samples.len(), 8_692_752);
    }

    #[test]
    fn the_folder_is_beside_the_workspace_by_default() {
        if std::env::var_os(RECORDINGS_ENV).is_none() {
            assert!(recordings_dir().ends_with("../sdrkit-recordings"));
        }
    }
}
