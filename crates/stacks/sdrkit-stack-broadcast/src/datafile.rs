//! A data file that a component loads at run time.

use sdrkit_core::{SettingError, SettingId, StorageError, StorePath};
use std::sync::Arc;

/// The error of a data file that a stack loads at run time.
pub trait FileError: Clone + std::fmt::Display {
    /// What the system said, if the file could not be read.
    fn read_kind(&self) -> Option<std::io::ErrorKind>;
}

/// A data file as a component holds it: the path that it came from, and its contents or why it was refused.
#[derive(Clone, Debug)]
pub struct Loaded<T, E> {
    path: String,
    value: Result<Option<Arc<T>>, E>,
}

impl<T, E: FileError> Loaded<T, E> {
    /// Loads the file at `path` with `read`. An empty path returns no contents and no error.
    pub fn load(path: &str, read: fn(&str) -> Result<T, E>) -> Self {
        let value = if path.is_empty() {
            Ok(None)
        } else {
            read(path).map(|t| Some(Arc::new(t)))
        };
        Loaded {
            path: path.into(),
            value,
        }
    }
    /// No file.
    pub fn none() -> Self {
        Loaded {
            path: String::new(),
            value: Ok(None),
        }
    }
    /// The path, empty for none.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// The contents, `None` for an empty path, or why the file was refused.
    pub fn value(&self) -> &Result<Option<Arc<T>>, E> {
        &self.value
    }
    /// The contents, if the file loaded.
    pub fn get(&self) -> Option<Arc<T>> {
        self.value.as_ref().ok().cloned().flatten()
    }
    /// Applies `path`, the value of setting `id`: reads it unless it is the path held. Returns true if the contents changed.
    ///
    /// # Errors
    ///
    /// [`SettingError::Storage`] if the file cannot be read, [`SettingError::Malformed`] if it is refused. What is held stays.
    pub fn apply(
        &mut self,
        id: SettingId,
        path: &str,
        read: fn(&str) -> Result<T, E>,
    ) -> Result<bool, SettingError> {
        if self.path == path {
            return Ok(false);
        }
        let loaded = Loaded::load(path, read);
        if let Err(e) = &loaded.value {
            return Err(match (e.read_kind(), StorePath::new(path)) {
                (Some(kind), Ok(path)) => SettingError::Storage {
                    id,
                    path,
                    source: StorageError::from(std::io::Error::from(kind)),
                },
                _ => SettingError::Malformed { id, at: None },
            });
        }
        *self = loaded;
        Ok(true)
    }
}
