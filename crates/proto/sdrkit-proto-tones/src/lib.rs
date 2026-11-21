//! Tone signalling: decoders and generators for CTCSS, DCS, DTMF, selcall, SELCAL, two-tone pages and MDC-1200 bursts. Each decoder reads
//! real audio at a rate that you give, decides by the share of the power that a tone holds, and reports through a callback.

#![deny(missing_docs)]

pub mod analyse;
pub mod components;
pub mod ctcss;
pub mod dcs;
pub mod dtmf;
mod error;
pub mod gen;
pub mod mdc;
pub mod selcal;
pub mod selcall;
pub mod subaudio;
pub mod twotone;

pub use error::{Param, TonesError};
pub use selcal::STACK_ID as SELCAL_STACK_ID;
