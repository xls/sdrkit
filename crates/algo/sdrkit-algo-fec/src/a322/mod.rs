//! The ATSC 3.0 bit interleavers of a PLP and the protection of the L1 signalling.
//!
//! Standard: ATSC A/322:2023-03 clause 6.2 (parity, group-wise and block interleavers) and clause 6.5 (L1-Basic and L1-Detail:
//! BCH, zero padding, LDPC, parity permutation, repetition, puncturing and bit demultiplexing).
//!
//! | Item | What it holds |
//! |---|---|
//! | [`BitInterleaver`] | the three interleavers of a PLP as one permutation of the LDPC code word |
//! | [`L1Code`] | one protection mode of L1-Basic or L1-Detail: encoder and soft decoder |
//! | [`tables`] | Annex B and tables 6.8, 6.9, 6.20 to 6.22 |

mod bil;
mod l1;
#[rustfmt::skip]
pub mod tables;

pub use bil::{block_interleaver, BitInterleaver, BlockType};
pub use l1::{L1Code, L1Kind, L1Lengths};
