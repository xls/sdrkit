//! LDPC code tables as published, kept as data: DVB-S2, DVB-S2X, DVB-T2 and ATSC 3.0 parity accumulator address tables, the
//! DVB BCH polynomials and parameters, the DTMB matrices (GB 20600-2006), the WSJT-family parity-check lists and the tables of
//! the FreeDV OFDM voice waveforms. Not
//! included: the CDR matrices (GY/T 268.1-2013 annex D, not published
//! in a free source); and the MSK144 (128,90), FST4 (240,101), FST4W (240,74) and Q65 tables (published only in GPL-licensed
//! software). A host loads these at run time with [`super::LdpcCode::from_checks`].

#![allow(clippy::unreadable_literal)]

pub mod atsc3;
pub mod dtmb;
pub mod dvb_bch;
pub mod dvb_s2;
pub mod dvb_s2x;
pub mod dvb_t2;
pub mod freedv;
pub mod wsjt;

/// A DVB (S2, S2X, T2) LDPC code as its standard prints it: for each group of 360 information bits, the parity accumulator
/// addresses of the group's first bit (EN 302 307-1 clause 5.3.2). Bit `m` of the group adds to parity `(x + m q) mod (n − k)`
/// for each address `x`; the parity bits are then accumulated (`p_i ^= p_(i−1)`).
#[derive(Debug)]
pub struct DvbTable {
    /// A name for the code.
    pub name: &'static str,
    /// Code word length `n_ldpc`.
    pub n: u32,
    /// Information bits `k_ldpc`.
    pub k: u32,
    /// `q = (n − k) / 360`.
    pub q: u32,
    /// One row per group of 360 information bits.
    pub rows: &'static [&'static [u16]],
}
