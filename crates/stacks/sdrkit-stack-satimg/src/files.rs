//! Files that a command names: the read through storage of the `path` argument ([`crate::typed::FileArgs`]).

use sdrkit_core::command::CommandError;
use sdrkit_core::{MemStorage, SettingError, StageError, Storage, StorePath, Values};
use std::sync::Arc;

/// The key of the argument that names the file.
const PATH: &str = "path";

/// The storage of a component until the run context supplies the host's: an empty store in memory.
pub fn in_memory() -> Arc<dyn Storage> {
    Arc::new(MemStorage::new())
}

/// Reads the file `text`, the `path` argument of the call `args`.
///
/// # Errors
///
/// [`CommandError::BadArgument`] with [`SettingError::Malformed`] if the text is not a storage path, and
/// [`CommandError::Failed`] with the [`sdrkit_core::StorageError`] if the read fails.
pub fn read(storage: &dyn Storage, args: &Values, text: &str) -> Result<Vec<u8>, CommandError> {
    let path = StorePath::new(text.trim()).map_err(|_| match args.schema().id(PATH) {
        Ok(arg) => CommandError::BadArgument {
            arg,
            error: SettingError::Malformed { id: arg, at: None },
        },
        Err(_) => CommandError::NotAnArgument { key: PATH },
    })?;
    storage.read(&path).map_err(|e| CommandError::Failed {
        source: StageError::new(e),
    })
}
