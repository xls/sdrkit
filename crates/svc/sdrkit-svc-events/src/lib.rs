//! What the event services share: observations of events, the standard fields of each event kind, the resolution of
//! names in user text, and the parts of an owner that every service uses.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod kinds;
pub mod observation;
pub mod plain;
pub mod service;
pub mod template;
pub mod text;

pub use kinds::{split_list, FamilySet, FieldRef, KindSet, Kinds, NameError, Standard};
pub use observation::{choice_key, choice_of_key, Context, Observation, Reading};
pub use sdrkit_util_geo::Position;
pub use template::Template;
