//! The ISDB-T channel coding pieces of one layer, all taken from the shared crates: the outer code RS(204,188),
//! the inner code with its puncturing, and the carrier mappings with unit mean power.
//!
//! Standard: ARIB STD-B31 clauses 3.3 to 3.9.

use super::tmcc::LayerParams;
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::conv::{codes, ConvCode, Puncture};
use sdrkit_algo_fec::rs::{codes as rs_codes, RsCode};
use sdrkit_algo_modem::Constellation;

/// The outer code of every layer: RS(204,188) shortened from RS(255,239). ARIB STD-B31 clause 3.3.
pub const OUTER: RsCode = rs_codes::DVB_204_188;

/// The inner code of every layer: K = 7, G1 = 171 octal sent first, G2 = 133 octal. ARIB STD-B31 clause 3.6.2.
pub const INNER: ConvCode = codes::ISDB_T_K7;

/// The pattern of rate 1/2: every coded value is sent.
const RATE_1_2: Puncture<'static> = Puncture::new(&[1, 1]);

/// The puncturing pattern of a code-rate code (0 = 1/2, 1 = 2/3, 2 = 3/4, 3 = 5/6, 4 = 7/8) over the serial stream X1 Y1 X2 Y2.
/// ARIB STD-B31 table 3-5 uses the patterns of DVB-T.
pub fn puncture(code_rate: u8) -> Puncture<'static> {
    match code_rate {
        0 => RATE_1_2,
        1 => codes::DVB_2_3,
        2 => codes::DVB_3_4,
        3 => codes::DVB_5_6,
        _ => codes::DVB_7_8,
    }
}

/// The points of a modulation code (1 = QPSK, 2 = 16QAM, 3 = 64QAM. 0 = DQPSK, whose cells are QPSK points once detected), with
/// unit mean power. The first bit of a label is the most significant: it sets the sign of I. ARIB STD-B31 clause 3.8.
pub fn points(modulation: u8) -> PointSet {
    let m = match modulation {
        2 => 16,
        3 => 64,
        _ => 4,
    };
    PointSet::qam(m, QamMap::DVB).unwrap_or_else(|_| PointSet::qpsk())
}

/// The demapper of a modulation code (see [`points`]).
pub fn constellation(modulation: u8) -> Constellation {
    Constellation::new(points(modulation))
        .unwrap_or_else(|_| Constellation::new(PointSet::qpsk()).unwrap_or_else(|_| unreachable!()))
}

/// The label (bits most significant first) of `bits` (one bit per byte).
#[inline]
pub fn label(bits: &[u8]) -> usize {
    bits.iter()
        .fold(0usize, |a, &b| (a << 1) | (b & 1) as usize)
}

/// Bits per cell of a layer.
pub fn bits_per_cell(p: &LayerParams) -> usize {
    p.bits() as usize
}
