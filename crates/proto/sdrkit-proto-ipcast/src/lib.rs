//! IP over broadcast: the layers that DVB-S2, DVB-T2 and ATSC 3.0 receivers share above their FEC, from baseband headers
//! and data fields to IP packets (GSE and MPE), ROUTE objects and their signalling packages, carousel file broadcasts and fragmented MP4, and the
//! `ipcast.*` graph components.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod adapt;
pub mod alp;
pub mod bbheader;
pub mod components;
mod error;
pub mod field;
pub mod filecast;
pub mod flow;
pub mod gse;
pub mod gzip;
pub mod ip;
pub mod lct;
pub mod merge;
pub mod mp4;
pub mod mpe;
pub mod pcap;
pub mod route;
pub mod sls;

pub use components::register;
pub use error::{Check, Error, Feature, Limit, Malformed, Result};
