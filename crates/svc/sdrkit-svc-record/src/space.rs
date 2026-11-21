//! The disk space rules: a reserve that stays free, and a budget for the recordings folder that deletes the oldest recordings
//! first. Each file counts once, by its path.

use crate::error::RecordError;
use crate::format::container_of;
use sdrkit_core::{Storage, StorageError, StorePath};
use std::collections::BTreeMap;

/// Makes sure that `need` bytes fit beside `reserve` bytes. A storage that does not know its free space passes.
///
/// # Errors
///
/// [`RecordError::NoSpace`] if they do not fit, [`RecordError::Storage`] if the storage fails.
pub(crate) fn check(storage: &dyn Storage, reserve: u64, need: u64) -> Result<(), RecordError> {
    match storage.free_space() {
        Ok(Some(free)) if free < reserve.saturating_add(need) => {
            Err(RecordError::NoSpace { need, free })
        }
        Ok(_) => Ok(()),
        Err(e) => Err(RecordError::storage(&StorePath::root(), e)),
    }
}

/// What a trim did.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TrimReport {
    /// Bytes of the recordings before.
    pub before: u64,
    /// Bytes of the recordings after.
    pub after: u64,
    /// The files deleted, oldest first.
    pub deleted: Vec<StorePath>,
    /// The files that did not delete.
    pub failed: Vec<(StorePath, StorageError)>,
}

/// One recording: its files share a stem (`x.sigmf-data` and `x.sigmf-meta`, `x.ts` and `x.json`).
#[derive(Debug, Default)]
struct Group {
    files: Vec<StorePath>,
    bytes: u64,
    newest: f64,
}

/// The stem that the files of one recording share: the name without its extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    }
}

/// True if `path` is a file of a recording: a recording container, or the metadata beside one.
fn is_recording_file(path: &StorePath) -> bool {
    container_of(path).is_some() || path.extension() == Some("json")
}

/// The files of the recordings in `folder` with their sizes and modification times. Other files are left out.
fn groups(storage: &dyn Storage, folder: &StorePath) -> Result<Vec<Group>, RecordError> {
    let entries = match storage.list(folder) {
        Ok(e) => e,
        Err(StorageError::NotFound) => return Ok(Vec::new()),
        Err(e) => return Err(RecordError::storage(folder, e)),
    };
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for e in entries.into_iter().filter(|e| !e.is_dir) {
        let Ok(path) = folder.join(&e.name) else {
            continue;
        };
        if !is_recording_file(&path) {
            continue;
        }
        let g = groups.entry(stem(&e.name).to_string()).or_default();
        g.files.push(path);
        g.bytes += e.size;
        g.newest = g.newest.max(e.modified.unwrap_or(0.0));
    }
    let mut list: Vec<Group> = groups.into_values().collect();
    list.sort_by(|a, b| a.newest.total_cmp(&b.newest));
    Ok(list)
}

/// The bytes of the recordings in `folder`.
pub(crate) fn folder_bytes(storage: &dyn Storage, folder: &StorePath) -> Result<u64, RecordError> {
    Ok(groups(storage, folder)?.iter().map(|g| g.bytes).sum())
}

/// Keeps the recordings in `folder` within `budget` bytes: the oldest go first (by the newest file of each), with all their
/// files. A recording with a file that `keep` accepts (one in progress) stays.
///
/// # Errors
///
/// [`RecordError::Storage`] if the folder does not list.
pub(crate) fn trim(
    storage: &dyn Storage,
    folder: &StorePath,
    budget: u64,
    keep: impl Fn(&StorePath) -> bool,
) -> Result<TrimReport, RecordError> {
    let list = groups(storage, folder)?;
    let before: u64 = list.iter().map(|g| g.bytes).sum();
    let mut rep = TrimReport {
        before,
        after: before,
        ..TrimReport::default()
    };
    for g in list {
        if rep.after <= budget {
            break;
        }
        if g.files.iter().any(&keep) {
            continue;
        }
        let mut ok = true;
        for f in g.files {
            match storage.remove(&f) {
                Ok(()) => rep.deleted.push(f),
                Err(e) => {
                    ok = false;
                    rep.failed.push((f, e));
                }
            }
        }
        if ok {
            rep.after = rep.after.saturating_sub(g.bytes);
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::MemStorage;

    #[test]
    fn oldest_recordings_go_first_with_their_metadata() {
        let s = MemStorage::new();
        let dir = StorePath::new("rec").unwrap();
        let mk = |name: &str, len: usize, t: f64| {
            s.set_time(t);
            let p = dir.join(name).unwrap();
            s.write(&p, &vec![0u8; len]).unwrap();
            p
        };
        mk("a_100.000MHz.sigmf-data", 1000, 100.0);
        mk("a_100.000MHz.sigmf-meta", 10, 100.0);
        mk("b.ts", 1000, 500.0);
        mk("b.json", 10, 500.0);
        let c = mk("c.wav", 1000, 900.0);
        mk("notes.txt", 5000, 0.0);
        assert_eq!(folder_bytes(&s, &dir).unwrap(), 3020);
        // Over budget by one recording: the oldest goes with its metadata.
        let r = trim(&s, &dir, 2500, |_| false).unwrap();
        assert_eq!((r.before, r.after, r.deleted.len()), (3020, 2010, 2));
        assert!(s.stat(&dir.join("notes.txt").unwrap()).is_ok());
        // A recording in progress stays even when it is the one over.
        let r = trim(&s, &dir, 500, |p| *p == c).unwrap();
        assert_eq!(r.after, 1000);
        assert!(s.stat(&c).is_ok());
        assert!(s.stat(&dir.join("b.json").unwrap()).is_err());
    }
}
