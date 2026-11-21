//! The memory service: memory channels, bookmarks on the spectrum and presets of the stacks, kept in the host's storage.
//! A host reads and changes them through [`MemoryService`] as an owner.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod bookmark;
mod error;
mod exchange;
mod files;
pub mod freq;
mod memory;
mod preset;
mod service;
mod store;

pub use error::{LineProblem, MemoryError, Problem};
pub use exchange::Format;
pub use memory::Memory;
pub use preset::Preset;
pub use service::{
    FilterKind, MemoryCommand, MemoryService, MemorySetting, OnSameChoice, Outcome, Part,
    BOOKMARKS, GROUPS, KEY, MEMORIES, MEMORY_CAP, MEMORY_GAINS, MEMORY_SETTINGS, MEMORY_TAGS,
    PRESETS, PRESET_AUDIO, PRESET_GAINS, PRESET_SETTINGS, PROBLEMS, ROW, SESSION, TAGS,
};
pub use store::{MemoryFilter, MemoryStore, OnSame, SortKey, Stored};
