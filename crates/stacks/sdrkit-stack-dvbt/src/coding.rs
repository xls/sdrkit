//! The DVB-T channel coding parameters, taken from the shared crates: the inner code and its puncturing, the outer code, the
//! interleavers, the energy dispersal sequence, the pilot sequence, the carrier tables and the constellations.
//!
//! Standard: ETSI EN 300 744 clauses 4.3 to 4.6.

use crate::params::{CodeRate, Constellation, Hierarchy, Mode};
use sdrkit_algo_dsp::constellation::{PointSet, QamMap};
use sdrkit_algo_fec::conv::{codes, ConvCode, Puncture};
use sdrkit_algo_modem::ofdm::{DVBT_2K_CONTINUAL, DVBT_2K_TPS};
use sdrkit_util_bits::interleave::Permutation;

pub use sdrkit_util_bits::interleave::DVBT_BIT_OFFSETS;

/// The inner code: rate 1/2, K = 7, X = 171 and Y = 133 octal, X sent first (clause 4.3.3).
pub const INNER: ConvCode = codes::DVB_K7;

/// Carriers between two repeats of the 2K pilot and TPS tables in the 8K mode (EN 300 744 tables 7 and 8).
const REPEAT: usize = 1704;

/// The puncturing pattern of a code rate (clause 4.3.3, table 2).
pub fn puncture(rate: CodeRate) -> Puncture<'static> {
    match rate {
        CodeRate::R12 => codes::NONE_1_2,
        CodeRate::R23 => codes::DVB_2_3,
        CodeRate::R34 => codes::DVB_3_4,
        CodeRate::R56 => codes::DVB_5_6,
        CodeRate::R78 => codes::DVB_7_8,
    }
}

/// The index in the mother-code pattern of the received bit `phase` of a puncturing period. For example, at rate 3/4 the
/// received bits are X1 Y1 Y2 X3, so received bit 2 is mother-code place 3.
pub fn pattern_index(rate: CodeRate, phase: usize) -> usize {
    puncture(rate)
        .keep()
        .iter()
        .enumerate()
        .filter(|(_, &k)| k == 1)
        .nth(phase)
        .map_or(0, |(i, _)| i)
}

/// The pilot sequence w_k for k = 0 to `n` - 1 (clause 4.5.2).
pub fn pilot_sequence(n: usize) -> Vec<u8> {
    let mut lfsr = sdrkit_util_bits::seq::DVBT_PILOTS;
    let mut w = vec![0u8; n];
    lfsr.fill_bits(&mut w);
    w
}

/// The continual pilots of the 1K mode (ETSI EN 302 583 table 5.27).
const CONTINUAL_1K: [u16; 25] = [
    0, 48, 54, 87, 141, 156, 192, 201, 255, 279, 282, 333, 432, 450, 483, 525, 531, 618, 636, 714,
    759, 765, 786, 813, 852,
];

/// The continual pilots of the 4K mode (ETSI EN 302 583 table 5.27; EN 300 744 Annex F).
const CONTINUAL_4K: [u16; 89] = [
    0, 48, 54, 87, 141, 156, 192, 201, 255, 279, 282, 333, 432, 450, 483, 525, 531, 618, 636, 714,
    759, 765, 780, 804, 873, 888, 918, 939, 942, 969, 984, 1050, 1101, 1107, 1110, 1137, 1140,
    1146, 1206, 1269, 1323, 1377, 1491, 1683, 1704, 1752, 1758, 1791, 1845, 1860, 1896, 1905, 1959,
    1983, 1986, 2037, 2136, 2154, 2187, 2229, 2235, 2322, 2340, 2418, 2463, 2469, 2484, 2508, 2577,
    2592, 2622, 2643, 2646, 2673, 2688, 2754, 2805, 2811, 2814, 2841, 2844, 2850, 2910, 2973, 3027,
    3081, 3195, 3387, 3408,
];

/// The TPS carriers of the 1K mode (ETSI EN 302 583 table 5.28).
const TPS_1K: [u16; 7] = [34, 209, 346, 413, 569, 688, 790];

/// The TPS carriers of the 4K mode (ETSI EN 302 583 table 5.28; EN 300 744 Annex F).
const TPS_4K: [u16; 34] = [
    34, 50, 209, 346, 413, 569, 595, 688, 790, 901, 1073, 1219, 1262, 1286, 1469, 1594, 1687, 1738,
    1754, 1913, 2050, 2117, 2273, 2299, 2392, 2494, 2605, 2777, 2923, 2966, 2990, 3173, 3298, 3391,
];

/// The continual pilot carriers of a mode, in increasing order. The 8K table repeats the 2K table every 1704 carriers
/// (tables 7 and 8 of clause 4.5.3 and 4.6); the 1K and 4K tables are those of ETSI EN 302 583 table 5.27.
pub fn continual(mode: Mode) -> Vec<usize> {
    match mode {
        Mode::K1 => CONTINUAL_1K.iter().map(|&k| k as usize).collect(),
        Mode::K4 => CONTINUAL_4K.iter().map(|&k| k as usize).collect(),
        Mode::K2 | Mode::K8 => {
            let mut v = repeated(&DVBT_2K_CONTINUAL, mode);
            v.dedup();
            v
        }
    }
}

/// The TPS carriers of a mode, in increasing order (clause 4.6, table 8; ETSI EN 302 583 table 5.28 for 1K and 4K).
pub fn tps_carriers(mode: Mode) -> Vec<usize> {
    match mode {
        Mode::K1 => TPS_1K.iter().map(|&k| k as usize).collect(),
        Mode::K4 => TPS_4K.iter().map(|&k| k as usize).collect(),
        Mode::K2 | Mode::K8 => repeated(&DVBT_2K_TPS, mode),
    }
}

fn repeated(table: &[u16], mode: Mode) -> Vec<usize> {
    let blocks = if mode == Mode::K2 { 1 } else { 4 };
    let mut v: Vec<usize> = (0..blocks)
        .flat_map(|b| table.iter().map(move |&k| k as usize + REPEAT * b))
        .collect();
    v.sort_unstable();
    v
}

/// The symbol interleaver H(q) of a mode: input `q` of an even symbol goes to carrier `H(q)` (clause 4.3.4.2; ETSI EN 302 583
/// clause 5.7.2 for 1K and 4K).
pub fn symbol_permutation(mode: Mode) -> Vec<u16> {
    match mode {
        Mode::K1 | Mode::K4 => symbol_interleaver(mode),
        Mode::K2 | Mode::K8 => Permutation::dvbt_symbol(mode == Mode::K8)
            .map(|p| p.dests().iter().map(|&d| d as u16).collect())
            .unwrap_or_default(),
    }
}

/// The register taps and bit permutation of the symbol interleaver of a mode (ETSI EN 302 583 tables 5.16 to 5.19): the R'
/// bits whose sum becomes the new top bit, and the R bit that R' bit `j` goes to.
fn interleaver_rule(mode: Mode) -> (&'static [u32], &'static [u32]) {
    match mode {
        Mode::K1 => (&[0, 5], &[4, 3, 0, 6, 2, 8, 1, 5, 7]),
        Mode::K2 => (&[0, 3], &[4, 3, 9, 6, 2, 8, 1, 5, 7, 0]),
        Mode::K4 => (&[0, 2], &[6, 3, 0, 9, 4, 2, 1, 8, 5, 10, 7]),
        Mode::K8 => (&[0, 1, 4, 6], &[7, 1, 4, 2, 9, 6, 8, 10, 0, 3, 11, 5]),
    }
}

/// The symbol interleaver H(q) of any mode from its register rule: R' of Nr - 1 bits is 0, 0, then 1, then shifts down with
/// the sum of its taps as the new top bit; H = (i mod 2) 2^(Nr - 1) + R, kept while below the data-carrier count.
pub fn symbol_interleaver(mode: Mode) -> Vec<u16> {
    let (taps, perm) = interleaver_rule(mode);
    let bits = perm.len() as u32;
    let nd = mode.data_carriers();
    let mut rp = 0u32;
    let mut h = Vec::with_capacity(nd);
    for i in 0..mode.fft() {
        rp = match i {
            0 | 1 => 0,
            2 => 1,
            _ => {
                let fb = taps.iter().fold(0, |a, &t| a ^ (rp >> t)) & 1;
                (rp >> 1) | (fb << (bits - 1))
            }
        };
        let r = perm
            .iter()
            .enumerate()
            .fold(0u32, |a, (j, &to)| a | ((rp >> j) & 1) << to);
        let v = ((i % 2) as u32) << bits | r;
        if (v as usize) < nd {
            h.push(v as u16);
        }
    }
    h
}

/// The points of a constellation and hierarchy at unit mean power (clause 4.3.5).
pub fn point_set(c: Constellation, h: Hierarchy) -> PointSet {
    let alpha = h.alpha() as u32;
    let set = match c {
        Constellation::Qpsk => PointSet::qam(4, QamMap::DVB),
        Constellation::Qam16 => PointSet::qam_hierarchical(16, alpha),
        Constellation::Qam64 => PointSet::qam_hierarchical(64, alpha),
    };
    set.unwrap_or_else(|_| unreachable!("every DVB-T constellation is a valid point set"))
}

/// The constellation of a mode, for hard decisions and soft demapping.
pub fn constellation(c: Constellation, h: Hierarchy) -> sdrkit_algo_modem::Constellation {
    sdrkit_algo_modem::Constellation::new(point_set(c, h))
        .unwrap_or_else(|_| unreachable!("every DVB-T point set is a valid constellation"))
}

/// The branch of each bit of a cell (clause 4.3.4.1). Non-hierarchical: one stream of v bits. Hierarchical: the HP stream on
/// branches 0 and 1, the LP stream on the others.
pub fn branches(c: Constellation, h: Hierarchy, lp: bool) -> &'static [usize] {
    match (c, h.hierarchical(), lp) {
        (Constellation::Qpsk, _, _) => &[0, 1],
        (_, true, false) => &[0, 1],
        (Constellation::Qam16, false, _) => &[0, 2, 1, 3],
        (Constellation::Qam16, true, true) => &[2, 3],
        (Constellation::Qam64, false, _) => &[0, 2, 4, 1, 3, 5],
        (Constellation::Qam64, true, true) => &[2, 4, 3, 5],
    }
}
