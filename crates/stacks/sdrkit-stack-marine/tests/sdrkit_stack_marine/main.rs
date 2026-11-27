//! The tests of the marine family: one module for each part, and the helpers they share.

mod common;

mod acceptance;
mod ais_crc;
mod ais_fit;
mod ais_vector;
mod atis_vector;
mod marine_vectors;
mod persistence;
mod plans;
mod roundtrip;
mod vdes_vector;
mod vhf_roundtrip;

/// Counts large allocations for `ais_fit` (only on the thread that enables it, so the other tests do not
/// disturb it).
#[global_allocator]
static ALLOC: ais_fit::Counting = ais_fit::Counting;
