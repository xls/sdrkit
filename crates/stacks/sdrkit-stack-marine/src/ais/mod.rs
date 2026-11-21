//! AIS: the line around each frame, the message layer and its NMEA output, the burst detector and decoder, the station table,
//! alerts, satellite analysis, the receiver graph and the generator.
//!
//! Standard: ITU-R M.1371-5

pub mod alerts;
pub mod binary;
pub mod demod;
pub mod display;
pub mod encode;
pub mod generator;
pub mod gmsk;
pub mod line;
pub mod message;
pub mod receiver;
pub mod satellite;
pub mod station;
