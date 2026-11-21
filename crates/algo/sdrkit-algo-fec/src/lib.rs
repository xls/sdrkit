//! Forward error correction: convolutional, Reed-Solomon, BCH, LDPC, turbo, polar and small block codes, with their decoders.
//!
//! The crate depends on `sdrkit-util-bits` only. Every code names its standard and clause.
//!
//! # Modules
//!
//! | Module | What it holds |
//! |---|---|
//! | [`llr`] | the `i8` LLR type, hard decisions, the shared [`Quantizer`] |
//! | [`error`] | the [`Decoded`] report and the [`FecError`] every decoder returns |
//! | [`conv`] | convolutional codes written as the standards write them, every named code and puncturing pattern, encoders, the Viterbi decoder (whole frame, streaming, tail-biting, NEON / SSE2 / AVX2 kernels with a scalar twin), trellis-coded modulation, Fano sequential decoding |
//! | [`gf`], [`rs`] | GF(2^m) for m = 2 .. 16; Reed-Solomon with any field, `fcr` and root spacing, shortening, both symbol orders, the CCSDS dual basis, interleaving and errors-and-erasures decoding, every named code |
//! | [`bch`], [`cyclic`] | binary BCH codes of any length (DVB-S2/T2 encoded a byte at a time), long cyclic encoders, majority-logic decoding of difference-set codes |
//! | [`reedmuller`] | the second-order Reed-Muller code RM(2,5) with its maximum-likelihood soft decoder |
//! | [`bauer`] | the Bauer (10,5) code of five-unit teleprinter characters |
//! | [`cauchy`] | a systematic erasure code over GF(256) with a Cauchy parity matrix |
//! | [`a322`] | the ATSC 3.0 bit interleavers of a PLP (parity, group-wise, block) and the L1-Basic and L1-Detail protection (A/322 clauses 6.2 and 6.5) |
//! | [`shorten`] | shortening and puncturing of a systematic code by groups of bits in a given order (DVB-T2 and DVB-C2 L1) |
//! | [`chase`] | Chase soft decoding over any hard decoder (BCH, short codes) with the margin to the next code word and the repair cost |
//! | [`small`] | short codes by syndrome table (Golay, Hamming, QR, short BCH, RDS, MPT 1327, TETRA Reed-Muller), Chase soft decoding, the least-reliable-bits search, product codes (BPTC), biorthogonal codes by fast Hadamard transform |
//! | [`mobitex`] | the Mobitex (12,8) byte code, its 12 by 20 block interleaver and the block scrambler |
//! | [`nbldpc`] | non-binary LDPC codes over GF(2^m) loaded as data (check lists, text form, repeat-accumulate shape), encoder by elimination, sum-product decoder over symbol probabilities with Walsh-Hadamard check nodes (the QRA codes of Q65) |
//! | [`ldpc`] | LDPC codes as quasi-cyclic layers: the DVB-S2 / S2X / T2 and ATSC 3.0 (A/322) tables, DTMB (GB 20600) codes from run-time tables, 5G NR base graphs, WSJT codes, codes with masked lanes; the alist reader and writer; a sparse encoder for codes of any size ([`ldpc::LdpcEncoder`]); layered min-sum with `i8` messages; sum-product with early stop and OSD |
//! | [`turbo`], [`ratematch`] | the LTE turbo code (QPP, max-log-MAP) and LTE rate matching with soft combining |
//! | [`turbo_codes`] | named configurations of the general turbo code (the 3GPP2 code of DVB-SH) |
//! | [`polar`] | NR polar codes with rate matching and CRC-aided list decoding |
//! | [`interleave`] | the Forney convolutional interleaver (writes before it reads), delay banks, FEC-chain permutations |
//! | [`blockinterleave`], [`branchdelay`] | convolutional interleavers of blocks of items, partial block permutations; the branch delays of the DVB-SH time interleaver |
//! | [`increments`] | the block interleaver of MIL-STD-188-141B C.5.1.2, written and read along walks of row and column increments |
//! | [`twistblock`] | the twisted row-column block interleaver with a mask of used positions (DVB-C2) |
//! | [`bitcells`], [`addrgen`] | the bit interleaver and cell-word demultiplexer of the second-generation DVB systems; the address generator of the DVB OFDM symbol and frequency interleavers |
//! | [`search`], [`par`] | hypothesis searches by re-encoding, and frame-parallel decoding on threads |
//! | [`tables_3gpp`] | the 3GPP tables (QPP, polar sequence and interleavers, NR LDPC base graphs) as published |
//! | [`voice`] | named tables of the digital voice stacks: NXDN puncturing, dPMR interleaves, NXDN vocoder test frames, vocoder frame layouts loaded at run time |
//!
//! Soft bits are LLRs `L = ln P(0) / P(1)`: a positive value means 0 and 0 is an erasure. Hard bits are `u8`, one per byte, the
//! first bit sent first. Bad lengths, parameters, erasures and NaN return a [`FecError`]. A failed decode leaves its input as
//! received unless the documentation says otherwise. Decoders return [`Decoded`] (`corrected`, `metric`, `iterations`).
//!
//! # Example
//!
//! ```
//! use sdrkit_algo_fec::conv::{codes, Encoder, Ends, Viterbi};
//! use sdrkit_algo_fec::rs::{codes as rs_codes, Rs};
//! // A DVB-style chain: RS(204,188) outside, the K = 7 code inside.
//! let mut rs = Rs::new(rs_codes::DVB_204_188).unwrap();
//! let mut packet = [0u8; 204];
//! packet[0] = 0x47;
//! rs.encode_word(&mut packet).unwrap();
//! let bits: Vec<u8> = packet.iter().flat_map(|b| (0..8).rev().map(move |i| (b >> i) & 1)).collect();
//! let mut enc = Encoder::new(codes::DVB_K7);
//! let mut coded = Vec::new();
//! enc.encode(&bits, &mut coded);
//! enc.terminate(&mut coded);
//! let llr: Vec<i8> = coded.iter().map(|&c| if c == 0 { 30 } else { -30 }).collect();
//! let mut v = Viterbi::new(codes::DVB_K7, 2000).unwrap();
//! let mut out = vec![0u8; bits.len()];
//! v.decode(&llr, Ends::Zero, &mut out).unwrap();
//! assert_eq!(out, bits);
//! ```

#![warn(missing_docs)]
#![deny(unsafe_code)]
// Kernels index several arrays by the same position, as the standards' equations do.
#![allow(clippy::needless_range_loop)]

pub mod a322;
pub mod addrgen;
pub mod bauer;
pub mod bch;
pub mod bitcells;
pub mod blockinterleave;
pub mod branchdelay;
pub mod cauchy;
pub mod chase;
pub mod conv;
pub mod cyclic;
pub mod digitrev;
pub mod error;
pub mod gf;
pub mod increments;
pub mod interleave;
pub mod ldpc;
pub mod llr;
pub mod mobitex;
pub mod nordstrom;
pub mod par;
pub mod polar;
pub mod ratematch;
pub mod reedmuller;
pub mod rs;
pub mod search;
pub mod shorten;
pub mod small;
pub mod syndromelist;
#[rustfmt::skip]
pub mod tables_3gpp;
pub mod nbldpc;
pub mod turbo;
pub mod turbo_codes;
pub mod twistblock;
pub mod voice;

pub use error::{Decoded, FecError, Param, Result, TextFault};
pub use llr::{Llr8, Quantizer};
