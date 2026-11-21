//! The numeric tables of DVB-C2 that are not formulas: FEC parameters, column twists, demultiplexers, XFECFrame lengths, the
//! L1 shortening and puncturing orders, continual pilots, reserved tones, the frequency interleaver rules and the Reed-Muller
//! header scrambling sequence.
//!
//! Standard: ETSI EN 302 769 V1.3.1 tables 3, 7, 10, 16(b), 28, 29, 32, 35, 36 and clause 7.2.

use sdrkit_algo_fec::bch::{codes, BchCode};
use sdrkit_algo_fec::ldpc::tables::{dvb_s2, dvb_t2, DvbTable};
use sdrkit_core::Choice;

sdrkit_core::choice_enum! {
    /// The constellation of a PLP (PLP_MOD).
    pub enum Qam {
        /// 16-QAM (001).
        Q16 = ("16", "16-QAM"),
        /// 64-QAM (010).
        Q64 = ("64", "64-QAM"),
        /// 256-QAM (011).
        Q256 = ("256", "256-QAM"),
        /// 1024-QAM (100).
        Q1024 = ("1024", "1024-QAM"),
        /// 4096-QAM (101).
        Q4096 = ("4096", "4096-QAM"),
    }
}

impl Qam {
    /// Bits per cell, eta_MOD.
    pub fn bits(self) -> usize {
        match self {
            Qam::Q16 => 4,
            Qam::Q64 => 6,
            Qam::Q256 => 8,
            Qam::Q1024 => 10,
            Qam::Q4096 => 12,
        }
    }
    /// Points.
    pub fn points(self) -> usize {
        1 << self.bits()
    }
    /// The PLP_MOD code.
    pub fn code(self) -> u8 {
        self.index() as u8 + 1
    }
    /// The constellation of a PLP_MOD code.
    pub fn from_code(c: u8) -> Option<Qam> {
        Some(match c {
            1 => Qam::Q16,
            2 => Qam::Q64,
            3 => Qam::Q256,
            4 => Qam::Q1024,
            5 => Qam::Q4096,
            _ => return None,
        })
    }
}

sdrkit_core::choice_enum! {
    /// The code rate of a PLP (PLP_COD).
    pub enum CodeRate {
        /// 2/3 (001).
        R2_3 = ("2_3", "2/3"),
        /// 3/4 (010).
        R3_4 = ("3_4", "3/4"),
        /// 4/5 (011).
        R4_5 = ("4_5", "4/5"),
        /// 5/6 (100).
        R5_6 = ("5_6", "5/6"),
        /// 9/10 with 64K frames, 8/9 with 16K frames (101).
        R9_10 = ("9_10", "9/10 (64K) or 8/9 (16K)"),
    }
}

impl CodeRate {
    /// The PLP_COD code.
    pub fn code(self) -> u8 {
        self.index() as u8 + 1
    }
    /// The rate of a PLP_COD code.
    pub fn from_code(c: u8) -> Option<CodeRate> {
        Some(match c {
            1 => CodeRate::R2_3,
            2 => CodeRate::R3_4,
            3 => CodeRate::R4_5,
            4 => CodeRate::R5_6,
            5 => CodeRate::R9_10,
            _ => return None,
        })
    }
    /// The rate as text for a frame size.
    pub fn text(self, long: bool) -> &'static str {
        match (self, long) {
            (CodeRate::R9_10, false) => "8/9",
            (CodeRate::R9_10, true) => "9/10",
            (r, _) => r.label(),
        }
    }
}

/// One PLP coding: frame size, constellation and code rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModCod {
    /// 64800-bit frames (PLP_FEC_TYPE 1); 16200-bit frames otherwise.
    pub long: bool,
    /// The constellation.
    pub qam: Qam,
    /// The code rate.
    pub rate: CodeRate,
}

impl ModCod {
    /// N_ldpc.
    pub fn n(&self) -> usize {
        if self.long {
            64_800
        } else {
            16_200
        }
    }
    /// Cells per FEC block (table 8).
    pub fn cells(&self) -> usize {
        self.n() / self.qam.bits()
    }
    /// Whether tables 11(a) and 11(b) list it.
    pub fn valid(&self) -> bool {
        use CodeRate::*;
        use Qam::*;
        matches!(
            (self.rate, self.qam),
            (R2_3, Q64)
                | (R3_4, Q256 | Q1024)
                | (R4_5, Q16 | Q64 | Q4096)
                | (R5_6, Q256 | Q1024 | Q4096)
                | (R9_10, _)
        )
    }
    /// The FEC parameters.
    pub fn fec(&self) -> Fec {
        fec(self.long, Some(self.rate))
    }
    /// "64K 256-QAM 5/6".
    pub fn text(&self) -> String {
        format!(
            "{} {} {}",
            if self.long { "64K" } else { "16K" },
            self.qam.label(),
            self.rate.text(self.long)
        )
    }
}

/// The FEC of one code (tables 3(a), 3(b)).
#[derive(Clone, Copy, Debug)]
pub struct Fec {
    /// N_ldpc.
    pub n: usize,
    /// K_ldpc.
    pub k_ldpc: usize,
    /// K_bch, the BBFRAME length.
    pub k_bch: usize,
    /// The BCH code.
    pub bch: &'static BchCode,
    /// The LDPC table.
    pub ldpc: &'static DvbTable,
}

/// The FEC of a frame size and rate; `None` is rate 1/2 of the 16K frame (the L1 signalling).
pub fn fec(long: bool, rate: Option<CodeRate>) -> Fec {
    use CodeRate::*;
    let (key, ldpc): (&str, &'static DvbTable) = match (long, rate) {
        (true, Some(R2_3)) => ("dvb_s2_normal_2_3", &dvb_t2::T2_N_2_3),
        (true, Some(R3_4)) => ("dvb_s2_normal_3_4", &dvb_s2::S2_N_3_4),
        (true, Some(R4_5)) => ("dvb_s2_normal_4_5", &dvb_s2::S2_N_4_5),
        (true, Some(R5_6)) => ("dvb_s2_normal_5_6", &dvb_s2::S2_N_5_6),
        (true, Some(R9_10)) => ("dvb_s2_normal_9_10", &dvb_s2::S2_N_9_10),
        (_, None) => ("dvb_s2_short_1_2", &dvb_s2::S2_S_1_2),
        (false, Some(R2_3)) => ("dvb_s2_short_2_3", &dvb_s2::S2_S_2_3),
        (false, Some(R3_4)) => ("dvb_s2_short_3_4", &dvb_s2::S2_S_3_4),
        (false, Some(R4_5)) => ("dvb_s2_short_4_5", &dvb_s2::S2_S_4_5),
        (false, Some(R5_6)) => ("dvb_s2_short_5_6", &dvb_s2::S2_S_5_6),
        (false, Some(R9_10)) => ("dvb_s2_short_8_9", &dvb_s2::S2_S_8_9),
    };
    let bch = codes::by_key(key).unwrap_or_else(|| unreachable!("every DVB-S2 BCH key is in the catalogue"));
    Fec {
        n: ldpc.n as usize,
        k_ldpc: ldpc.k as usize,
        k_bch: bch.k,
        bch,
        ldpc,
    }
}

/// The column twist t_c (tables 7(a), 7(b)); its length is the number of columns.
pub fn twist(qam: Qam, long: bool) -> &'static [u16] {
    match (qam, long) {
        (Qam::Q16, true) => &[0, 0, 2, 4, 4, 5, 7, 7],
        (Qam::Q16, false) => &[0, 0, 0, 1, 7, 20, 20, 21],
        (Qam::Q64, true) => &[0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9],
        (Qam::Q64, false) => &[0, 0, 0, 2, 2, 2, 3, 3, 3, 6, 7, 7],
        (Qam::Q256, true) => &[0, 2, 2, 2, 2, 3, 7, 15, 16, 20, 22, 22, 27, 27, 28, 32],
        (Qam::Q256, false) => &[0, 0, 0, 1, 7, 20, 20, 21],
        (Qam::Q1024, true) => &[
            0, 1, 3, 4, 5, 6, 6, 9, 13, 14, 14, 16, 21, 21, 23, 25, 25, 26, 28, 30,
        ],
        (Qam::Q1024, false) => &[0, 0, 0, 2, 2, 2, 2, 2, 5, 5, 5, 5, 5, 7, 7, 7, 7, 8, 8, 10],
        (Qam::Q4096, true) => &[0, 0, 2, 2, 3, 4, 4, 5, 5, 7, 8, 9],
        (Qam::Q4096, false) => &[
            0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 2, 3, 7, 9, 9, 9, 10, 10, 10, 10, 10, 11,
        ],
    }
}

/// The demultiplexer (tables 10(a), 10(b)): input bit `i` goes to sub-stream `table[i mod len]`.
pub fn demux(qam: Qam, long: bool, rate: Option<CodeRate>) -> &'static [u8] {
    match (qam, long) {
        (Qam::Q16, _) => &[7, 1, 4, 2, 5, 3, 6, 0],
        (Qam::Q64, _) => &[11, 7, 3, 10, 6, 2, 9, 5, 1, 8, 4, 0],
        (Qam::Q256, true) if rate == Some(CodeRate::R2_3) => {
            &[7, 2, 9, 0, 4, 6, 13, 3, 14, 10, 15, 5, 8, 12, 11, 1]
        }
        (Qam::Q256, true) => &[15, 1, 13, 3, 8, 11, 9, 5, 10, 6, 4, 7, 12, 2, 14, 0],
        (Qam::Q256, false) => &[7, 3, 1, 5, 2, 6, 4, 0],
        (Qam::Q1024, true) => &[
            8, 16, 7, 19, 4, 15, 3, 12, 0, 11, 10, 9, 13, 2, 14, 5, 17, 6, 18, 1,
        ],
        (Qam::Q1024, false) => &[
            8, 3, 7, 10, 19, 4, 9, 5, 17, 6, 14, 11, 2, 18, 16, 15, 0, 1, 13, 12,
        ],
        (Qam::Q4096, true) => &[8, 0, 6, 1, 4, 5, 2, 3, 7, 10, 11, 9],
        (Qam::Q4096, false) => &[
            10, 15, 4, 19, 21, 16, 23, 18, 11, 14, 22, 5, 6, 17, 13, 20, 1, 3, 9, 2, 7, 8, 12, 0,
        ],
    }
}

/// XFECFRAME_LENGTH in cells (table 16(b)) for PLP_FEC_TYPE and PLP_MOD; `None` for a code with no length.
pub fn xfecframe_len(long: bool, plp_mod: u8) -> Option<usize> {
    let t: [usize; 8] = if long {
        [900, 16_200, 10_800, 8100, 6480, 5400, 4629, 4050]
    } else {
        [0, 4050, 2700, 2025, 1620, 1350, 1158, 1013]
    };
    t.get(usize::from(plp_mod)).copied().filter(|&n| n > 0)
}

/// The cells of a stuffing Data Slice Packet (PLP_MOD 000, PLP_FEC_TYPE 1).
pub const STUFFING_CELLS: usize = 900;

/// The order in which the 20 groups of 360 information bits of the L1 code are padded (table 28).
pub const L1_PAD_ORDER: [u8; 20] = [18, 17, 16, 15, 14, 13, 12, 11, 4, 10, 9, 8, 7, 3, 2, 1, 6, 5, 19, 0];

/// The order in which the 25 parity groups of the L1 code are punctured (table 29).
pub const L1_PUNCTURE_ORDER: [u8; 25] = [
    6, 4, 13, 9, 18, 8, 15, 20, 5, 17, 2, 22, 24, 7, 12, 1, 16, 23, 14, 0, 21, 10, 19, 11, 3,
];

/// The continual pilots, as carrier offsets within an L1 block (table 35).
pub const CONTINUAL: [u16; 30] = [
    96, 216, 306, 390, 450, 486, 780, 804, 924, 1026, 1224, 1422, 1554, 1620, 1680, 1902, 1956, 2016, 2142, 2220, 2310,
    2424, 2466, 2736, 3048, 3126, 3156, 3228, 3294, 3366,
];

/// The reserved tone set S0 over 8 L1 blocks (table 36).
pub const RESERVED_S0: [u16; 288] = [
    161, 243, 296, 405, 493, 584, 697, 741, 821, 934, 1021, 1160, 1215, 1312, 1417, 1462, 1591, 1693, 1729, 1845, 1910,
    1982, 2127, 2170, 2339, 2365, 2499, 2529, 2639, 2745, 2864, 2950, 2992, 3119, 3235, 3255, 3559, 3620, 3754, 3835,
    3943, 3975, 4061, 4210, 4270, 4371, 4417, 4502, 4640, 4677, 4822, 4904, 5026, 5113, 5173, 5271, 5317, 5426, 5492,
    5583, 5740, 5757, 5839, 5935, 6033, 6146, 6212, 6369, 6454, 6557, 6597, 6711, 6983, 7047, 7173, 7202, 7310, 7421,
    7451, 7579, 7666, 7785, 7831, 7981, 8060, 8128, 8251, 8326, 8369, 8445, 8569, 8638, 8761, 8873, 8923, 9017, 9104,
    9239, 9283, 9368, 9500, 9586, 9683, 9782, 9794, 9908, 9989, 10123, 10327, 10442, 10535, 10658, 10739, 10803, 10925,
    11006, 11060, 11198, 11225, 11326, 11474, 11554, 11663, 11723, 11810, 11902, 11987, 12027, 12117, 12261, 12320,
    12419, 12532, 12646, 12676, 12808, 12915, 12941, 13067, 13113, 13246, 13360, 13426, 13520, 13811, 13862, 13936,
    14073, 14102, 14206, 14305, 14408, 14527, 14555, 14650, 14755, 14816, 14951, 15031, 15107, 15226, 15326, 15392,
    15484, 15553, 15623, 15734, 15872, 15943, 16043, 16087, 16201, 16299, 16355, 16444, 16514, 16635, 16723, 16802,
    16912, 17150, 17285, 17387, 17488, 17533, 17603, 17708, 17793, 17932, 18026, 18081, 18159, 18285, 18356, 18395,
    18532, 18644, 18697, 18761, 18874, 18937, 19107, 19119, 19251, 19379, 19414, 19522, 19619, 19691, 19748, 19875,
    19935, 20065, 20109, 20261, 20315, 20559, 20703, 20737, 20876, 20950, 21069, 21106, 21231, 21323, 21379, 21494,
    21611, 21680, 21796, 21805, 21958, 22027, 22091, 22167, 22324, 22347, 22459, 22551, 22691, 22761, 22822, 22951,
    22981, 23089, 23216, 23290, 23402, 23453, 23529, 23668, 23743, 24019, 24057, 24214, 24249, 24335, 24445, 24554,
    24619, 24704, 24761, 24847, 24947, 25089, 25205, 25274, 25352, 25474, 25537, 25612, 25711, 25748, 25874, 25984,
    26078, 26155, 26237, 26324, 26378, 26545, 26623, 26720, 26774, 26855, 26953, 27021, 27123,
];

/// The register taps of the frequency interleaver (clause 9.4.5): R'[10] = R'[0] xor R'[2].
pub const FREQ_TAPS: [u32; 2] = [0, 2];

/// The bit of R that bit `j` of R' goes to, for H0 (even symbols) and H1 (odd symbols) (table 32).
pub const FREQ_PERM: [[u32; 11]; 2] = [
    [6, 3, 0, 9, 4, 2, 1, 8, 5, 10, 7],
    [5, 9, 1, 4, 3, 0, 8, 10, 7, 2, 6],
];

/// The default of the word w^RM that scrambles the lower branch of a Reed-Muller header (clause 7.2.2.3), w^RM_0 the most
/// significant bit: 0 0 0 1 1 1 0 1 1 0 0 0 1 1 0 1 0 0 1 1 1 0 1 1 1 1 1 1 0 0 1 1 as the printed page of EN 302 769 V1.3.1
/// shows it. One source only: the stack keeps it as the setting `rm_scrambling`.
pub const W_RM_DEFAULT: u32 = 0x1D8D_3BF3;
