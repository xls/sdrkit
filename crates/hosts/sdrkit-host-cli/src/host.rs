//! The engine of the tool, its storage, the saved settings, and the owner names of the command line.

use crate::error::CliError;
use sdrkit_core::storage::{StoreReader, StoreWriter};
use sdrkit_core::{
    Facilities, Owner, Storage, StorageError, StoreEntry, StorePath, Values, WriteMode,
};
use sdrkit_graph::ComponentHandle;
use sdrkit_host_engine::{
    system_clock, system_random, DirStorage, Engine, EngineError, OwnerRef, RadioId, SessionId,
    Setup, StdNet,
};
use std::path::Path;
use std::sync::Arc;

/// The file in the storage that keeps the saved settings between runs.
const SETTINGS: &str = "settings.bin";

/// The folder of the storage under which [`Mounted`] shows a folder of the computer.
pub const OUTSIDE: &str = "outside";

/// Makes the engine with every stack, component kind and driver, on `storage` and the USB bus `usb`, and applies the
/// saved settings.
///
/// # Errors
///
/// What [`Engine::with_usb`] returns, and [`CliError::Engine`] if the saved settings do not apply.
pub fn engine(
    storage: Arc<dyn Storage>,
    usb: Arc<dyn sdrkit_driver_usb::UsbBus>,
) -> Result<Arc<Engine>, CliError> {
    let facilities = Facilities {
        storage: Arc::clone(&storage),
        clock: system_clock(),
        random: system_random(),
        codecs: Arc::new(sdrkit_host_catalog::codecs()),
    };
    let components = sdrkit_host_catalog::registry(&facilities);
    let engine = Engine::with_usb(
        Setup {
            facilities,
            transport: Arc::new(StdNet),
            stacks: sdrkit_host_catalog::all(),
            families: sdrkit_host_catalog::families(),
            components,
        },
        usb,
    )?;
    let path = settings_path()?;
    match storage.read(&path) {
        Ok(bytes) => engine.import_settings(&bytes)?,
        Err(StorageError::NotFound) => {}
        Err(e) => return Err(e.into()),
    }
    Ok(Arc::new(engine))
}

fn settings_path() -> Result<StorePath, StorageError> {
    StorePath::new(SETTINGS).map_err(StorageError::InvalidPath)
}

/// Writes the saved settings of the engine into the storage, so the next run starts with them. The settings of radios and
/// sessions are in them too; the next run skips those that are not open when it starts.
///
/// # Errors
///
/// What the export and the storage give.
pub fn save_settings(engine: &Engine, storage: &dyn Storage) -> Result<(), CliError> {
    storage.write(&settings_path()?, &engine.export_settings()?)?;
    Ok(())
}

/// What `run` started: the owners named `radio`, `session` and `stack` on its console.
#[derive(Clone, Copy, Debug)]
pub struct Live {
    /// The radio it opened.
    pub radio: Option<RadioId>,
    /// The session it started.
    pub session: SessionId,
    /// The stack the session runs.
    pub stack: Option<&'static str>,
}

/// The owner that `name` names: `engine`, `radio`, `session`, `stack`, `service:<key>`, `kind:<catalog>:<key>` (catalog
/// `stack`, `driver` or `component`), `component:<handle>`, or an owner key of the engine (`service.scan`, `stack.fm`). In
/// `run`, `stack.<id>` of the stack that runs names the stack of the session.
///
/// # Errors
///
/// [`CliError::UnknownOwner`], or [`CliError::NotRunning`] for an owner of `run` outside it.
pub fn owner(engine: &Engine, name: &str, live: Option<&Live>) -> Result<OwnerRef, CliError> {
    let unknown = || CliError::UnknownOwner {
        name: name.to_owned(),
    };
    let running = || {
        live.ok_or_else(|| CliError::NotRunning {
            name: name.to_owned(),
        })
    };
    let find = |key: &str| match engine.find(key) {
        Ok(r) => Ok(r),
        Err(EngineError::NoSuchKey { .. } | EngineError::BadKey(_)) => Err(unknown()),
        Err(e) => Err(e.into()),
    };
    match name {
        "engine" => return Ok(OwnerRef::Engine),
        "radio" => return running()?.radio.map(OwnerRef::Radio).ok_or_else(unknown),
        "session" => return Ok(OwnerRef::Session(running()?.session)),
        "stack" => return Ok(OwnerRef::Stack(running()?.session)),
        _ => {}
    }
    if let Some(key) = name.strip_prefix("service:") {
        return find(&format!("service.{key}"));
    }
    if let Some(rest) = name.strip_prefix("kind:") {
        let (catalog, key) = rest.split_once(':').ok_or_else(unknown)?;
        return match catalog {
            "stack" | "driver" | "component" => find(&format!("{catalog}.{key}")),
            _ => Err(unknown()),
        };
    }
    if let Some(n) = name.strip_prefix("component:") {
        let handle = n.parse::<u32>().map_err(|_| unknown())?;
        return Ok(OwnerRef::Component(
            running()?.session,
            ComponentHandle(handle),
        ));
    }
    if let (Some(l), Some(id)) = (live, name.strip_prefix("stack.")) {
        if l.stack == Some(id) {
            return Ok(OwnerRef::Stack(l.session));
        }
    }
    find(name)
}

/// The owner names a user can type, for the hint after an unknown name.
pub fn owner_names(engine: &Engine) -> Vec<String> {
    let mut v = vec!["engine".to_owned()];
    let services = engine.owner(OwnerRef::Engine, |o| {
        table_keys(o, sdrkit_host_engine::SERVICES.key)
    });
    v.extend(
        services
            .unwrap_or_default()
            .into_iter()
            .map(|k| format!("service:{}", k.trim_start_matches("service."))),
    );
    v.push("kind:stack:<id>, kind:driver:<key>, kind:component:<kind>".into());
    v.push("radio, session, stack, component:<handle> (on the console of `run`)".into());
    v
}

/// The row keys of table `key` of `o`, oldest first.
pub fn table_keys(o: &dyn Owner, key: &str) -> Vec<String> {
    let r = o.results();
    let Some(t) = r.decl().table_id(key).and_then(|id| r.table(id)) else {
        return Vec::new();
    };
    let mut keys: Vec<String> = t.rows_newest_first().map(|(k, _)| k.to_owned()).collect();
    keys.reverse();
    keys
}

/// The default values of the schema of owner `r`.
///
/// # Errors
///
/// [`CliError::Engine`] if `r` does not refer to an owner.
pub fn defaults(engine: &Engine, r: OwnerRef) -> Result<Values, CliError> {
    Ok(engine.owner(r, |o| Values::defaults(o.schema()))?)
}

/// The host's storage with one folder of the computer shown read-only at `outside/`, so the file driver can play a
/// recording that is not in the storage.
pub struct Mounted {
    main: Arc<dyn Storage>,
    outside: DirStorage,
}

impl Mounted {
    /// `main`, with the folder `folder` at `outside/`.
    ///
    /// # Errors
    ///
    /// What [`DirStorage::new`] returns.
    pub fn new(main: Arc<dyn Storage>, folder: &Path) -> Result<Self, CliError> {
        Ok(Mounted {
            main,
            outside: DirStorage::new(folder)?,
        })
    }

    /// The path in the outside folder, if `p` is below `outside/`.
    fn outside(p: &StorePath) -> Option<StorePath> {
        let rest = p.as_str().strip_prefix(OUTSIDE)?.strip_prefix('/')?;
        StorePath::new(rest).ok()
    }
}

impl Storage for Mounted {
    fn read(&self, path: &StorePath) -> Result<Vec<u8>, StorageError> {
        match Self::outside(path) {
            Some(p) => self.outside.read(&p),
            None => self.main.read(path),
        }
    }
    fn write(&self, path: &StorePath, data: &[u8]) -> Result<(), StorageError> {
        match Self::outside(path) {
            Some(_) => Err(StorageError::Denied),
            None => self.main.write(path, data),
        }
    }
    fn open_writer(
        &self,
        path: &StorePath,
        how: WriteMode,
    ) -> Result<Box<dyn StoreWriter>, StorageError> {
        match Self::outside(path) {
            Some(_) => Err(StorageError::Denied),
            None => self.main.open_writer(path, how),
        }
    }
    fn open_reader(&self, path: &StorePath) -> Result<Box<dyn StoreReader>, StorageError> {
        match Self::outside(path) {
            Some(p) => self.outside.open_reader(&p),
            None => self.main.open_reader(path),
        }
    }
    fn remove(&self, path: &StorePath) -> Result<(), StorageError> {
        match Self::outside(path) {
            Some(_) => Err(StorageError::Denied),
            None => self.main.remove(path),
        }
    }
    fn remove_tree(&self, path: &StorePath) -> Result<(), StorageError> {
        match Self::outside(path) {
            Some(_) => Err(StorageError::Denied),
            None => self.main.remove_tree(path),
        }
    }
    fn rename(&self, from: &StorePath, to: &StorePath) -> Result<(), StorageError> {
        match (Self::outside(from), Self::outside(to)) {
            (None, None) => self.main.rename(from, to),
            _ => Err(StorageError::Denied),
        }
    }
    fn list(&self, folder: &StorePath) -> Result<Vec<StoreEntry>, StorageError> {
        match Self::outside(folder) {
            Some(p) => self.outside.list(&p),
            None => self.main.list(folder),
        }
    }
    fn free_space(&self) -> Result<Option<u64>, StorageError> {
        self.main.free_space()
    }
    fn stat(&self, path: &StorePath) -> Result<StoreEntry, StorageError> {
        match Self::outside(path) {
            Some(p) => self.outside.stat(&p),
            None => self.main.stat(path),
        }
    }
}
