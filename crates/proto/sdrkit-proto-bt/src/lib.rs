//! The Bluetooth BR/EDR baseband, as a receiver of the basic rate reads it.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`baseband`] | channels, sync words from a LAP, access codes and their search, the packet header and its HEC, FEC 1/3 and 2/3, whitening and the payload CRC |
//! | [`fhs`] | the FHS packet: its fields, the device address, and the whole packet after the access code |
//! | [`cod`] | the class of device as text |
//!
//! Standard: Bluetooth Core Specification v5.4, Vol 2 Part B (Baseband) clauses 6 and 7; Bluetooth Assigned Numbers (Class of
//! Device).

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod baseband;
pub mod cod;
pub mod fhs;
pub mod packet;
