//! A data file that a text setting names by its path in the host's storage (a channel plan, a band plan): the path, the
//! storage once the run context gives it, and what was read from the file, or why it was not.
//!
//! The setting may come before the storage. The file is then read when [`DataFile::attach`] gives the storage. A path that
//! is not a storage path, or a file that its reader refuses, is an error of the setting; a file that cannot be read yet is not.

use sdrkit_core::{SettingError, SettingId, Storage, StorePath};
use std::sync::Arc;

/// A data file named by a setting (module documentation).
pub struct DataFile<T> {
    path: String,
    id: Option<SettingId>,
    storage: Option<Arc<dyn Storage>>,
    read: fn(&str) -> Result<T, String>,
    value: Option<Arc<T>>,
    fault: Option<String>,
}

impl<T> Clone for DataFile<T> {
    fn clone(&self) -> Self {
        DataFile {
            path: self.path.clone(),
            id: self.id,
            storage: self.storage.clone(),
            read: self.read,
            value: self.value.clone(),
            fault: self.fault.clone(),
        }
    }
}

impl<T> DataFile<T> {
    /// No file yet; `read` turns the text of a file into its contents, or says why it cannot.
    pub fn new(read: fn(&str) -> Result<T, String>) -> Self {
        DataFile {
            path: String::new(),
            id: None,
            storage: None,
            read,
            value: None,
            fault: None,
        }
    }

    /// The path, empty for none.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The contents, when the file was read.
    pub fn get(&self) -> Option<&Arc<T>> {
        self.value.as_ref()
    }

    /// Why a named file is not read: it cannot be read yet, or the storage is not there yet. `None` with no path or a file.
    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    /// Reads the file at the path from the storage.
    fn read_now(&self, storage: &dyn Storage, id: SettingId) -> Result<T, SettingError> {
        let path =
            StorePath::new(&self.path).map_err(|_| SettingError::Malformed { id, at: None })?;
        let bytes = storage
            .read(&path)
            .map_err(|source| SettingError::Storage { id, path, source })?;
        (self.read)(&String::from_utf8_lossy(&bytes))
            .map_err(|_| SettingError::Malformed { id, at: None })
    }

    /// Takes the value `path` of setting `id`. Returns whether the contents changed.
    ///
    /// # Errors
    ///
    /// [`SettingError::Malformed`] if `path` is not a storage path or the reader refuses the file. What is held stays.
    pub fn set_path(&mut self, id: SettingId, path: &str) -> Result<bool, SettingError> {
        let path = path.trim();
        self.id = Some(id);
        if path.is_empty() {
            self.path.clear();
            self.fault = None;
            return Ok(self.value.take().is_some());
        }
        if path == self.path && self.value.is_some() {
            return Ok(false);
        }
        if StorePath::new(path).is_err() {
            return Err(SettingError::Malformed { id, at: None });
        }
        let old = std::mem::replace(&mut self.path, path.to_owned());
        let read = self.storage.as_deref().map(|s| self.read_now(s, id));
        match read {
            Some(Ok(v)) => {
                self.value = Some(Arc::new(v));
                self.fault = None;
            }
            Some(Err(SettingError::Storage { source, .. })) => {
                self.value = None;
                self.fault = Some(source.to_string());
            }
            Some(Err(e)) => {
                self.path = old;
                return Err(e);
            }
            None => {
                self.value = None;
                self.fault = Some("the host's storage is not there yet".into());
            }
        }
        Ok(true)
    }

    /// Takes the host's storage and reads a file that waits for it. Returns whether the contents changed.
    pub fn attach(&mut self, storage: Arc<dyn Storage>) -> bool {
        self.storage = Some(storage);
        let (Some(id), Some(s)) = (self.id, self.storage.clone()) else {
            return false;
        };
        if self.path.is_empty() || self.value.is_some() {
            return false;
        }
        match self.read_now(&*s, id) {
            Ok(v) => {
                self.value = Some(Arc::new(v));
                self.fault = None;
                true
            }
            Err(SettingError::Storage { source, .. }) => {
                self.fault = Some(source.to_string());
                false
            }
            Err(_) => {
                self.fault = Some("not a file of the expected format".into());
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdrkit_core::MemStorage;

    fn number(text: &str) -> Result<u32, String> {
        text.trim().parse().map_err(|_| "not a number".to_string())
    }

    fn storage() -> Arc<MemStorage> {
        let s = MemStorage::new();
        s.write(&StorePath::new("plans/seven.txt").unwrap(), b"7")
            .unwrap();
        s.write(&StorePath::new("plans/word.txt").unwrap(), b"seven")
            .unwrap();
        Arc::new(s)
    }

    #[test]
    fn a_file_named_before_the_storage_is_read_when_it_comes() {
        let mut f = DataFile::new(number);
        let id = SettingId(3);
        assert!(f.set_path(id, "plans/seven.txt").unwrap());
        assert!(f.get().is_none());
        assert!(f.fault().is_some());
        assert!(f.attach(storage()));
        assert_eq!(f.get().map(|v| **v), Some(7));
        assert_eq!(f.fault(), None);
    }

    #[test]
    fn a_refused_file_or_path_is_an_error_and_the_contents_stay() {
        let mut f = DataFile::new(number);
        let id = SettingId(1);
        f.attach(storage());
        f.set_path(id, "plans/seven.txt").unwrap();
        assert!(matches!(
            f.set_path(id, "plans/word.txt"),
            Err(SettingError::Malformed { .. })
        ));
        assert!(f.set_path(id, "../outside").is_err());
        assert_eq!(f.get().map(|v| **v), Some(7));
        assert_eq!(f.path(), "plans/seven.txt");
        // A file that is not there yet is not an error: the fault says why.
        assert!(f.set_path(id, "plans/later.txt").unwrap());
        assert!(f.get().is_none());
        assert!(f.fault().is_some());
        // An empty path is no file.
        assert!(!f.set_path(id, "").unwrap());
        assert_eq!((f.path(), f.fault()), ("", None));
    }
}
