//! Acceleration backends: the [`Backend`] trait that every kernel family extends, and the [`Registry`] that holds the backends
//! of one family in the order of preference for the platform, with the plain backend last.

use std::sync::atomic::{AtomicUsize, Ordering};

/// What every backend of a kernel family has: a name and an availability check. A family trait (for example
/// `FftBackend` of sdrkit-algo-dsp) extends it with the kernel calls.
pub trait Backend: Sync {
    /// A short, stable name, for example `"plain"`.
    fn name(&self) -> &'static str;
    /// Whether the backend can run on this machine. The default is true.
    fn available(&self) -> bool {
        true
    }
}

/// The name given to [`Registry::select`] is not a backend of the family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownBackend;

impl std::fmt::Display for UnknownBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("no backend of the family has this name")
    }
}

impl std::error::Error for UnknownBackend {}

/// The backends of one kernel family, in the order of preference for this platform. The last one is the plain backend, which
/// runs everywhere. An object of the family takes the selected backend when it is built. If that backend is not available or
/// cannot do the job, the object takes the next one, and the plain backend last.
pub struct Registry<B: ?Sized + 'static> {
    table: &'static [&'static B],
    selected: AtomicUsize,
}

impl<B: ?Sized + Backend> Registry<B> {
    /// A registry of `table`, with the first entry selected. The last entry must be the plain backend.
    pub const fn new(table: &'static [&'static B]) -> Self {
        assert!(
            !table.is_empty(),
            "a backend registry needs its plain backend"
        );
        Registry {
            table,
            selected: AtomicUsize::new(0),
        }
    }

    /// The names of the registered backends, the default first and the plain backend last.
    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        self.table.iter().map(|b| b.name())
    }

    /// Selects the backend called `name` for the objects built from now on. Objects built before keep their backend. If the
    /// backend is not available, the objects take the next one. An error when no backend has this name.
    pub fn select(&self, name: &str) -> Result<(), UnknownBackend> {
        let i = self
            .table
            .iter()
            .position(|b| b.name() == name)
            .ok_or(UnknownBackend)?;
        self.selected.store(i, Ordering::Relaxed);
        Ok(())
    }

    /// The name of the selected backend.
    pub fn selected(&self) -> &'static str {
        self.table[self.selected.load(Ordering::Relaxed)].name()
    }

    /// The name of the backend that runs: the selected one if it is available, else the next available one.
    pub fn active(&self) -> &'static str {
        self.candidates()
            .next()
            .map_or_else(|| self.plain().name(), |b| b.name())
    }

    /// The available backends from the selected one to the plain one, in the order an object tries them.
    pub fn candidates(&self) -> impl Iterator<Item = &'static B> {
        let from = self.selected.load(Ordering::Relaxed);
        self.table[from..].iter().copied().filter(|b| b.available())
    }

    /// Every registered backend, available or not.
    pub fn all(&self) -> &'static [&'static B] {
        self.table
    }

    /// The plain backend.
    pub fn plain(&self) -> &'static B {
        self.table[self.table.len() - 1]
    }

    /// Whether `backend` is the plain backend (the names in a registry are unique).
    pub fn is_plain(&self, backend: &B) -> bool {
        backend.name() == self.plain().name()
    }

    /// The first candidate for which `make` returns a value, and that value. The plain backend is tried last, also when it
    /// reports itself as not available.
    pub fn build<T>(&self, mut make: impl FnMut(&'static B) -> Option<T>) -> Option<T> {
        self.candidates()
            .find_map(&mut make)
            .or_else(|| make(self.plain()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named(&'static str, bool);
    impl Backend for Named {
        fn name(&self) -> &'static str {
            self.0
        }
        fn available(&self) -> bool {
            self.1
        }
    }

    static FAST: Named = Named("fast", false);
    static PLAIN: Named = Named("plain", true);
    static TABLE: &[&Named] = &[&FAST, &PLAIN];

    #[test]
    fn selection_falls_back_to_the_plain_backend() {
        let r: Registry<Named> = Registry::new(TABLE);
        assert_eq!(r.names().collect::<Vec<_>>(), ["fast", "plain"]);
        assert_eq!(r.selected(), "fast");
        assert_eq!(r.active(), "plain");
        assert_eq!(r.select("none"), Err(UnknownBackend));
        assert_eq!(r.select("plain"), Ok(()));
        assert_eq!(r.selected(), "plain");
        assert_eq!(r.build(|b| (b.name() == "fast").then_some(1)), None);
        assert_eq!(r.build(|b| Some(b.name())), Some("plain"));
    }
}
