//! The channel offset of ISDB-T.

/// The ISDB-T signal is this far above the centre of its channel (ARIB STD-B31). A channel list that the host gives
/// includes it in the frequency of each channel.
pub const ISDBT_OFFSET_HZ: f64 = 1e6 / 7.0;
