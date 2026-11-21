//! The LRIT and HRIT file layer (together xRIT): files sent over CCSDS space packets. [`transport`] assembles files from
//! [`crate::packet::Packet`]s and splits them for generators. [`headers`] reads and writes the header records in the CGMS, NOAA
//! (GOES), KMA (GK-2A) and FY-4 dialects. [`crate::rice`] decodes the Rice-compressed image lines.
//!
//! Standard: CGMS 03, LRIT/HRIT Global Specification.

pub mod headers;
pub mod transport;
