//! The channel bank: several channels cut from one wide input in one pass of a shared polyphase filterbank, each decoded by a
//! receiver in a session of its own, and spans that decode every carrier of a raster. A host reads and controls it through
//! [`BankService`] as an owner. The engine runs its components ([`Channelize`], [`CarrierScan`]) on the wide input.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

mod component;
mod detect;
mod error;
pub mod pfb;
mod plan;
mod service;
mod spec;

pub use component::{
    parse_raster, register, CarrierScan, CarrierScanSetting, ChannelOut, Channelize, CARRIER_OFF,
    CARRIER_ON,
};
pub use detect::{CarrierDetector, Gate, Reading};
pub use error::BankError;
pub use service::{
    BankCommand, BankService, BankSetting, ChannelChange, ChannelState, PathKind, SquelchChange,
    CARRIERS, CHANNELS, KEY, SPANS,
};
pub use spec::{ChannelSpec, Raster, SpanOpen, SpanSpec};
