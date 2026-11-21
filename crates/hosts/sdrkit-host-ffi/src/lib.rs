//! The C API of sdrkit. cbindgen writes the header `include/sdrkit.h` from this crate.
//!
//! Every object (the engine, a radio, a session, the stack of a session, a component, a service, a type of a catalog) has
//! the same calls for its settings, results, events and commands. Streams have their own calls.
//!
//! The feature `presets` adds the calls `sdrkit_presets_*` over a data folder. Their header `include/sdrkit_presets.h`
//! is written by hand.

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]

mod byname;
mod commands;
mod decl;
mod engine;
mod error;
mod events;
mod graph;
mod handle;
/// cbindgen:ignore
#[cfg(feature = "presets")]
mod presets;
mod results;
mod settings;
mod storage;
mod streams;
mod typed;
#[cfg(test)]
mod tests;
mod value;

pub use byname::*;
pub use commands::*;
pub use decl::*;
pub use engine::*;
pub use error::*;
pub use events::*;
pub use graph::*;
pub use handle::Handle;
#[cfg(feature = "presets")]
pub use presets::*;
pub use results::*;
pub use settings::*;
pub use storage::*;
pub use streams::*;
pub use typed::*;
pub use value::*;
