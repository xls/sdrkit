//! Handles of owners and the table that holds the handle of each open radio and each session its handle.

use sdrkit_graph::ComponentHandle;
use std::collections::BTreeMap;
use std::sync::Arc;

/// An open radio. A closed radio's handle is invalid, also after its slot is used again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RadioId(Handle);

/// A session. A closed session's handle is invalid, also after its slot is used again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId(Handle);

/// A host feed: samples that the host pushes, which any number of sessions read. A closed feed's handle is invalid,
/// also after its slot is used again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FeedId(Handle);

/// A share: the samples of one radio served to network clients. A closed share's handle is invalid, also after its
/// slot is used again.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShareId(Handle);

/// The number of an owner in its table. A table numbers its owners from 1 and never uses a number twice.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Handle(u64);

impl Handle {
    fn to_u64(self) -> u64 {
        self.0
    }

    fn from_u64(v: u64) -> Self {
        Handle(v)
    }
}

macro_rules! handle_number {
    ($t:ident) => {
        impl $t {
            /// The handle as one number, as the row keys of the engine's tables give it.
            pub fn to_u64(self) -> u64 {
                self.0.to_u64()
            }
            /// The handle of a number that [`Self::to_u64`] returned.
            pub fn from_u64(v: u64) -> Self {
                $t(Handle::from_u64(v))
            }
        }
    };
}

handle_number!(RadioId);
handle_number!(SessionId);
handle_number!(FeedId);
handle_number!(ShareId);

/// One owner of the engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OwnerRef {
    /// The engine itself: the catalogs and the lists of radios and sessions.
    Engine,
    /// An open radio.
    Radio(RadioId),
    /// A session: the settings, results and commands of its graph, with its own counts.
    Session(SessionId),
    /// The stack of a stack session, with the stack's own declarations.
    Stack(SessionId),
    /// One component of the graph of a session.
    Component(SessionId, ComponentHandle),
    /// A stack kind, by its row in the engine's `stacks` table.
    StackKind(u32),
    /// A driver kind, by its row in the engine's `drivers` table.
    DriverKind(u32),
    /// A component kind, by its row in the engine's `component_kinds` table.
    ComponentKind(u32),
    /// A service, by its row in the engine's `services` table.
    Service(u32),
    /// A share of a radio.
    Share(ShareId),
}

/// A table of one kind of owner. Each owner has a number that the table never uses again, so the handle of an owner
/// that left stays invalid.
pub(crate) struct Slots<T> {
    items: BTreeMap<u64, Arc<T>>,
    last: u64,
}

impl<T> Default for Slots<T> {
    fn default() -> Self {
        Slots {
            items: BTreeMap::new(),
            last: 0,
        }
    }
}

impl<T> Slots<T> {
    pub(crate) fn insert(&mut self, item: Arc<T>) -> Handle {
        self.last += 1;
        self.items.insert(self.last, item);
        Handle(self.last)
    }

    pub(crate) fn get(&self, h: Handle) -> Option<&Arc<T>> {
        self.items.get(&h.0)
    }

    pub(crate) fn remove(&mut self, h: Handle) -> Option<Arc<T>> {
        self.items.remove(&h.0)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (Handle, &Arc<T>)> {
        self.items.iter().map(|(n, item)| (Handle(*n), item))
    }
}

impl RadioId {
    pub(crate) fn new(h: Handle) -> Self {
        RadioId(h)
    }
    pub(crate) fn handle(self) -> Handle {
        self.0
    }
}

impl SessionId {
    pub(crate) fn new(h: Handle) -> Self {
        SessionId(h)
    }
    pub(crate) fn handle(self) -> Handle {
        self.0
    }
}

impl FeedId {
    pub(crate) fn new(h: Handle) -> Self {
        FeedId(h)
    }
    pub(crate) fn handle(self) -> Handle {
        self.0
    }
}

impl ShareId {
    pub(crate) fn new(h: Handle) -> Self {
        ShareId(h)
    }
    pub(crate) fn handle(self) -> Handle {
        self.0
    }
}
