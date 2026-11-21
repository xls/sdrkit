//! Packet links: HDLC framing, AX.25 frames, KISS, APRS, D-PRS, the AFSK 1200 and G3RUH 9600 modems, an AFSK receiver for any tone pair, CSP with the
//! AX100 framings, a sync word hunter for burst links, and link CRCs without a catalogue name.
//!
//! Bad input returns an [`Error`], never a panic.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod aprs;
pub mod ax25;
pub mod components;
pub mod crc;
pub mod csp;
pub mod dprs;
mod error;
pub mod ft12;
pub mod hdlc;
pub mod hunt;
pub mod kiss;
pub mod modem;
pub mod s5066;

pub use error::{Error, Param, Part, Result};
