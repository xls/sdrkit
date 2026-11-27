//! The message store of sdrkit: everything decoded, kept and searchable, with exports, daily files and the session book.
//! It runs on the engine's event thread, never on a decoding thread.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod daily;
pub mod error;
pub mod export;
mod journal;
pub mod message;
pub mod session;
pub mod store;
mod time;

pub use daily::{DailyFiles, DailyFormat};
pub use error::StoreError;
pub use export::{export, Export};
pub use message::Message;
pub use session::{
    Session, SessionBook, SessionChange, SessionCommand, SessionSetting, SessionType, TrackPoint,
};
pub use store::{Added, Query, Store, StoreCommand, StoreSetting, KEY};
