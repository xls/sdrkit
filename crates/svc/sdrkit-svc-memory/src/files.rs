//! The files of the memory service in the host's storage. A file that cannot be read is moved aside
//! (`<name>.unreadable-<n>`) before a save can replace it.

use crate::error::MemoryError;
use crate::memory::Memory;
use crate::store::MemoryStore;
use sdrkit_core::{Storage, StorageError, StorePath};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

/// The version written into the files.
pub(crate) const FILE_VERSION: u32 = 1;

fn storage_error(path: &StorePath, source: StorageError) -> MemoryError {
    MemoryError::Storage {
        path: path.clone(),
        source,
    }
}

/// Writes `value` as JSON to `path`, as one step.
///
/// # Errors
///
/// [`MemoryError::Encode`] or [`MemoryError::Storage`].
pub(crate) fn save_json<T: Serialize + ?Sized>(
    storage: &dyn Storage,
    path: &StorePath,
    value: &T,
) -> Result<(), MemoryError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| MemoryError::Encode)?;
    storage
        .write(path, &bytes)
        .map_err(|e| storage_error(path, e))
}

/// What a load found.
#[derive(Debug)]
pub(crate) struct Loaded<T> {
    /// The data. The default if the file was missing or could not be read.
    pub value: T,
    /// The new path of a file that could not be read.
    pub set_aside: Option<StorePath>,
}

/// Reads JSON from `path`. A missing file produces the default. A file that does not parse is renamed to
/// `<name>.unreadable-<n>` (the first free `n`), and the default is returned.
///
/// # Errors
///
/// [`MemoryError::Storage`] if the storage fails.
pub(crate) fn load_json<T: DeserializeOwned + Default>(
    storage: &dyn Storage,
    path: &StorePath,
) -> Result<Loaded<T>, MemoryError> {
    let bytes = match storage.read(path) {
        Ok(b) => b,
        Err(StorageError::NotFound) => {
            return Ok(Loaded {
                value: T::default(),
                set_aside: None,
            })
        }
        Err(e) => return Err(storage_error(path, e)),
    };
    match serde_json::from_slice::<T>(&bytes) {
        Ok(value) => Ok(Loaded {
            value,
            set_aside: None,
        }),
        Err(_) => Ok(Loaded {
            value: T::default(),
            set_aside: Some(set_aside(storage, path)?),
        }),
    }
}

fn set_aside(storage: &dyn Storage, path: &StorePath) -> Result<StorePath, MemoryError> {
    for n in 1..10_000 {
        let name = format!("{}.unreadable-{n}", path.file_name());
        let aside = match path.parent() {
            Some(dir) => dir.join(&name),
            None => StorePath::new(&name),
        }
        .map_err(|e| storage_error(path, StorageError::InvalidPath(e)))?;
        match storage.stat(&aside) {
            Err(StorageError::NotFound) => {
                storage
                    .rename(path, &aside)
                    .map_err(|e| storage_error(path, e))?;
                return Ok(aside);
            }
            Err(e) => return Err(storage_error(&aside, e)),
            Ok(_) => {}
        }
    }
    Err(storage_error(path, StorageError::AlreadyExists))
}

/// The memory file: `{"version": 1, "memories": [...]}`. A bare list of memories is read as well.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct MemoryFile {
    memories: Vec<Memory>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AnyMemoryList {
    File(MemoryFile),
    List(Vec<Memory>),
}

impl Default for AnyMemoryList {
    fn default() -> Self {
        AnyMemoryList::List(Vec::new())
    }
}

/// Saves the memories of `store` to `path`.
///
/// # Errors
///
/// As [`save_json`].
pub(crate) fn save_memories(
    storage: &dyn Storage,
    path: &StorePath,
    store: &MemoryStore,
) -> Result<(), MemoryError> {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        memories: &'a [Memory],
    }
    save_json(
        storage,
        path,
        &Out {
            version: FILE_VERSION,
            memories: store.memories(),
        },
    )
}

/// What a load of the memory file found.
#[derive(Debug)]
pub(crate) struct LoadedMemories {
    /// The store.
    pub store: MemoryStore,
    /// The new path of a file that could not be read.
    pub set_aside: Option<StorePath>,
}

/// Loads the memory file into a store that holds at most `cap` memories.
///
/// # Errors
///
/// As [`load_json`].
pub(crate) fn load_memories(
    storage: &dyn Storage,
    path: &StorePath,
    cap: usize,
) -> Result<LoadedMemories, MemoryError> {
    let l = load_json::<AnyMemoryList>(storage, path)?;
    let list = match l.value {
        AnyMemoryList::File(f) => f.memories,
        AnyMemoryList::List(v) => v,
    };
    let (store, _dropped) = MemoryStore::from_memories(list, cap);
    Ok(LoadedMemories {
        store,
        set_aside: l.set_aside,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::MemStorage;

    fn path(p: &str) -> StorePath {
        StorePath::new(p).unwrap()
    }

    #[test]
    fn save_and_load_round_trip() {
        let st = MemStorage::new();
        let p = path("memory/memories.json");
        let mut s = MemoryStore::new();
        s.insert(Memory::new("Tower", 118.1e6, "general"), 1.0)
            .unwrap();
        save_memories(&st, &p, &s).unwrap();
        let l = load_memories(&st, &p, 100).unwrap();
        assert_eq!(l.store.memories(), s.memories());
        assert!(l.set_aside.is_none());
        let left = st.list(&path("memory")).unwrap();
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn missing_file_is_empty() {
        let st = MemStorage::new();
        let l = load_memories(&st, &path("none.json"), 100).unwrap();
        assert!(l.store.is_empty() && l.set_aside.is_none());
    }

    #[test]
    fn unreadable_file_is_set_aside_before_a_save() {
        let st = MemStorage::new();
        let p = path("memories.json");
        st.write(&p, b"{ not json").unwrap();
        let l = load_memories(&st, &p, 100).unwrap();
        let aside = l.set_aside.expect("set aside");
        assert_eq!(st.read(&aside).unwrap(), b"{ not json");
        save_memories(&st, &p, &l.store).unwrap();
        assert_eq!(st.read(&aside).unwrap(), b"{ not json", "kept");
        st.write(&p, b"[").unwrap();
        let l2 = load_memories(&st, &p, 100).unwrap();
        assert_ne!(l2.set_aside.unwrap(), aside);
    }

    #[test]
    fn bare_list_and_unknown_fields() {
        let st = MemStorage::new();
        let p = path("m.json");
        st.write(
            &p,
            br#"[{"name":"A","freq_hz":1e6,"stack":"fm","from_the_future":true},{"name":"B","freq_hz":2e6}]"#,
        )
        .unwrap();
        let l = load_memories(&st, &p, 1).unwrap();
        assert_eq!(l.store.len(), 1);
    }

    #[test]
    fn a_storage_failure_is_typed() {
        let st = MemStorage::new();
        let p = path("memories.json");
        st.fail_next(StorageError::Denied);
        assert_eq!(
            save_memories(&st, &p, &MemoryStore::new()),
            Err(MemoryError::Storage {
                path: p,
                source: StorageError::Denied
            })
        );
    }
}
