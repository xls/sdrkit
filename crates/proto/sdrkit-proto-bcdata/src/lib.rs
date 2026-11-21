//! Broadcast data and audio packaging shared by DAB, DRM and HD Radio: MSC data groups, packet mode, MOT and the slideshow,
//! Journaline, SPI, the dynamic label with DL Plus and RT+ tags, AAC packaging, DAB Layer II framing, one audio playout, and
//! [`publish`], the fields and tables that a stack shows of this data.

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod aac;
pub mod component;
pub mod datagroup;
mod error;
pub mod journaline;
pub mod label;
pub mod mot;
pub mod mp2;
pub mod packet;
pub mod playout;
pub mod publish;
pub mod slideshow;
pub mod spi;
pub mod tags;

pub use error::{Error, Malformed, Parameter, Result};

/// Adds the components of this crate (`bcdata.playout`) to a registry.
pub fn register(r: &mut sdrkit_graph::Registry) {
    component::register(r);
}
