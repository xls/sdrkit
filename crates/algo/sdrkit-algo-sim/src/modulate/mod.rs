//! Clean modulators: the generic signal shapes that many standards share, so a stack's generator only maps its own bits.
//!
//! | Type | Signals |
//! |---|---|
//! | [`Linear`] | BPSK, QPSK, 8PSK, QAM, APSK, ASK and PAM with any pulse of `sdrkit_algo_dsp::pulse` (RRC, RC, Gaussian, half-sine, rectangle) at any samples per symbol; OQPSK, and MSK as OQPSK with half-sine pulses |
//! | [`Cpm`] | continuous-phase FSK: CPFSK, MSK, GMSK, GFSK, multi-level FSK with any frequency pulse, with held edges and amplitude [`Ramp`]s for bursts |
//! | [`Afsk`] | audio FSK (Bell 202, DSC, SAME, MPT 1327) as real audio |
//! | [`Ook`] | on-off keying from timed intervals with raised-cosine edges centred on each change |
//! | [`Morse`] | Morse text keyed onto a carrier, once or repeating (a [`sdrkit_core::Generator`]) |
//! | [`Sweep`] | a linear frequency sweep, once or as a sawtooth with a pause (FMCW radars, ionosondes, receiver test sweeps) |
//! | [`Am`], [`Fm`], [`Ssb`] | audio to complex baseband: AM with or without carrier, FM with pre-emphasis, upper or lower sideband |
//!
//! Symbols for [`Linear`] come from [`crate::points`] (or a standard's own mapping); levels for [`Cpm`] are `+-1` (or `+-1, +-3`).
//! Every modulator keeps its state between calls, so a stream fed in pieces of any size comes out the same.

mod analog;
mod cpm;
mod keying;
mod linear;
mod pm;
mod sweep;

pub use analog::{Am, Fm, Sideband, Ssb};
pub use cpm::{Afsk, Cpm, Ramp};
pub use keying::{Morse, Ook};
pub use linear::Linear;
pub use pm::PhaseMod;
pub use sweep::Sweep;
