//! Aeronautical data link messages: ACARS, ADS-C and CPDLC, and the tables that keep what they say about each aircraft.
//!
//! This crate holds what VHF ACARS, VDL Mode 2, HFDL and the satellite links share. Each link's own modulation, framing and
//! headers stay in its stack.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod acars;
pub mod adsc;
pub mod air;
pub mod aircraft;
pub mod arinc622;
pub mod cpdlc;
mod error;
pub mod formats;
pub mod labels;
pub mod msglog;
pub mod readings;
pub mod textpos;
pub mod trail;

pub use error::{Error, FramingByte, Item, Result};
