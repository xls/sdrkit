//! The DVB-T2 parameter catalogue: FFT sizes, guard intervals, pilot patterns, constellations, code rates, frame sizes and
//! channel widths, with their signalling codes and the rules that join them.
//!
//! Standard: ETSI EN 302 755 V1.4.1 clauses 6, 7.2, 8.3 and 9, annex I.

use sdrkit_algo_fec::ldpc::tables::{dvb_s2, dvb_t2, DvbTable};
use sdrkit_core::{Choice, ChoiceDecl};

/// Implements [`Choice`] for a parameter enum: its variants in declaration order, each with its key and label.
macro_rules! parameter_choice {
    ($t:ident, [$(($v:ident, $key:literal, $label:literal)),+ $(,)?]) => {
        impl Choice for $t {
            const CHOICES: &'static [ChoiceDecl] = &[$(ChoiceDecl::new($key, $label)),+];
            fn index(self) -> u16 {
                self as u16
            }
            fn from_index(index: u16) -> Option<Self> {
                [$($t::$v),+].get(usize::from(index)).copied()
            }
        }
    };
}

parameter_choice!(
    Fft,
    [
        (K1, "1k", "1K"),
        (K2, "2k", "2K"),
        (K4, "4k", "4K"),
        (K8, "8k", "8K"),
        (K16, "16k", "16K"),
        (K32, "32k", "32K")
    ]
);
parameter_choice!(
    Guard,
    [
        (G1_32, "1/32", "1/32"),
        (G1_16, "1/16", "1/16"),
        (G1_8, "1/8", "1/8"),
        (G1_4, "1/4", "1/4"),
        (G1_128, "1/128", "1/128"),
        (G19_128, "19/128", "19/128"),
        (G19_256, "19/256", "19/256"),
    ]
);
parameter_choice!(
    PilotPattern,
    [
        (Pp1, "pp1", "PP1"),
        (Pp2, "pp2", "PP2"),
        (Pp3, "pp3", "PP3"),
        (Pp4, "pp4", "PP4"),
        (Pp5, "pp5", "PP5"),
        (Pp6, "pp6", "PP6"),
        (Pp7, "pp7", "PP7"),
        (Pp8, "pp8", "PP8"),
    ]
);
parameter_choice!(
    Modulation,
    [
        (Bpsk, "bpsk", "BPSK"),
        (Qpsk, "qpsk", "QPSK"),
        (Qam16, "16qam", "16-QAM"),
        (Qam64, "64qam", "64-QAM"),
        (Qam256, "256qam", "256-QAM"),
    ]
);
parameter_choice!(
    CodeRate,
    [
        (R1_2, "1/2", "1/2"),
        (R3_5, "3/5", "3/5"),
        (R2_3, "2/3", "2/3"),
        (R3_4, "3/4", "3/4"),
        (R4_5, "4/5", "4/5"),
        (R5_6, "5/6", "5/6"),
        (R1_3, "1/3", "1/3"),
        (R2_5, "2/5", "2/5"),
        (R1_4, "1/4", "1/4"),
    ]
);
parameter_choice!(
    FecFrame,
    [
        (Short, "16200", "16 200 bits"),
        (Normal, "64800", "64 800 bits")
    ]
);
parameter_choice!(
    Bandwidth,
    [
        (B1_7, "1.7", "1.7 MHz"),
        (B5, "5", "5 MHz"),
        (B6, "6", "6 MHz"),
        (B7, "7", "7 MHz"),
        (B8, "8", "8 MHz"),
        (B10, "10", "10 MHz"),
    ]
);
parameter_choice!(
    Profile,
    [(Base, "base", "T2-base"), (Lite, "lite", "T2-Lite")]
);

/// The FFT size of the symbols after P1 (clause 9.5, table 66).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Fft {
    /// 1K.
    K1,
    /// 2K.
    K2,
    /// 4K.
    K4,
    /// 8K.
    K8,
    /// 16K.
    K16,
    /// 32K.
    K32,
}

impl Fft {
    /// Every FFT size, smallest first.
    pub const ALL: [Fft; 6] = [Fft::K1, Fft::K2, Fft::K4, Fft::K8, Fft::K16, Fft::K32];

    /// The number of FFT points.
    pub fn size(self) -> usize {
        1024 << self.row()
    }

    /// The row of the size in the tables of the standard: 0 for 1K up to 5 for 32K.
    pub fn row(self) -> usize {
        self as usize
    }

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }

    /// Whether the extended carrier mode exists for this size (8K, 16K and 32K, clause 9.5).
    pub fn has_extended(self) -> bool {
        self >= Fft::K8
    }

    /// K_ext, the carriers added on each side in extended carrier mode (table 66).
    pub fn k_ext(self) -> usize {
        match self {
            Fft::K8 => 48,
            Fft::K16 => 144,
            Fft::K32 => 288,
            _ => 0,
        }
    }

    /// K_total in normal carrier mode (table 66).
    pub fn k_normal(self) -> usize {
        [853, 1705, 3409, 6817, 13633, 27265][self.row()]
    }

    /// K_total in the given carrier mode.
    pub fn k_total(self, extended: bool) -> usize {
        self.k_normal() + if extended { 2 * self.k_ext() } else { 0 }
    }

    /// N_P2, the number of P2 symbols (table 51).
    pub fn n_p2(self) -> usize {
        [16, 8, 4, 2, 1, 1][self.row()]
    }

    /// The continual pilot groups used with this size and K_mod (table 61). K_mod is `None` for 32K.
    pub fn cp_groups(self) -> (usize, Option<usize>) {
        match self {
            Fft::K1 => (1, Some(1632)),
            Fft::K2 => (2, Some(1632)),
            Fft::K4 => (3, Some(3264)),
            Fft::K8 => (4, Some(6528)),
            Fft::K16 => (5, Some(13056)),
            Fft::K32 => (6, None),
        }
    }

    /// A_CP, the continual pilot amplitude (table 62).
    pub fn a_cp(self) -> f32 {
        match self {
            Fft::K1 | Fft::K2 => 4.0 / 3.0,
            Fft::K4 => 4.0 * std::f32::consts::SQRT_2 / 3.0,
            _ => 8.0 / 3.0,
        }
    }

    /// The largest number of symbols after P1 in a 250 ms frame at 8 MHz (table 46), by guard interval.
    pub fn max_l_f(self, guard: Guard) -> Option<usize> {
        let row: [usize; 7] = match self {
            Fft::K32 => [68, 66, 64, 64, 60, 60, 0],
            Fft::K16 => [138, 135, 131, 129, 123, 121, 111],
            Fft::K8 => [276, 270, 262, 259, 247, 242, 223],
            Fft::K4 => [0, 540, 524, 0, 495, 0, 446],
            Fft::K2 => [0, 1081, 1049, 0, 991, 0, 892],
            Fft::K1 => [0, 0, 2098, 0, 1982, 0, 1784],
        };
        Some(row[guard.column()]).filter(|&n| n > 0)
    }
}

/// The guard interval fraction (clause 9.7, table 67) with its L1-pre code (table 22).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Guard {
    /// 1/32.
    G1_32,
    /// 1/16.
    G1_16,
    /// 1/8.
    G1_8,
    /// 1/4.
    G1_4,
    /// 1/128.
    G1_128,
    /// 19/128.
    G19_128,
    /// 19/256.
    G19_256,
}

impl Guard {
    /// Every guard interval, in the order of their L1-pre codes 0 to 6.
    pub const ALL: [Guard; 7] = [
        Guard::G1_32,
        Guard::G1_16,
        Guard::G1_8,
        Guard::G1_4,
        Guard::G1_128,
        Guard::G19_128,
        Guard::G19_256,
    ];

    /// The GUARD_INTERVAL code of table 22.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// The guard interval of a GUARD_INTERVAL code, or `None` for the reserved code.
    pub fn from_code(c: u8) -> Option<Guard> {
        Guard::ALL.get(c as usize).copied()
    }

    /// The fraction as numerator and denominator.
    pub fn fraction(self) -> (usize, usize) {
        match self {
            Guard::G1_32 => (1, 32),
            Guard::G1_16 => (1, 16),
            Guard::G1_8 => (1, 8),
            Guard::G1_4 => (1, 4),
            Guard::G1_128 => (1, 128),
            Guard::G19_128 => (19, 128),
            Guard::G19_256 => (19, 256),
        }
    }

    /// The fraction as a number.
    pub fn value(self) -> f64 {
        let (n, d) = self.fraction();
        n as f64 / d as f64
    }

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }

    /// The column of tables 46, 58 and 67 (1/128, 1/32, 1/16, 19/256, 1/8, 19/128, 1/4).
    pub fn column(self) -> usize {
        match self {
            Guard::G1_128 => 0,
            Guard::G1_32 => 1,
            Guard::G1_16 => 2,
            Guard::G19_256 => 3,
            Guard::G1_8 => 4,
            Guard::G19_128 => 5,
            Guard::G1_4 => 6,
        }
    }

    /// The guard samples (elementary periods) for an FFT size, or `None` where table 67 says NA.
    pub fn samples(self, fft: Fft) -> Option<usize> {
        let allowed = match fft {
            Fft::K32 => self != Guard::G1_4,
            Fft::K16 | Fft::K8 => true,
            Fft::K4 | Fft::K2 => !matches!(self, Guard::G1_128 | Guard::G19_256 | Guard::G19_128),
            Fft::K1 => matches!(self, Guard::G1_16 | Guard::G1_8 | Guard::G1_4),
        };
        let (n, d) = self.fraction();
        allowed.then_some(fft.size() * n / d)
    }

    /// Whether this guard interval belongs to the set that S2 field 1 signals together with the "19" family (1/128, 19/256,
    /// 19/128) for 8K and 32K (table 19(a)).
    pub fn is_short_family(self) -> bool {
        matches!(self, Guard::G1_128 | Guard::G19_256 | Guard::G19_128)
    }
}

/// A scattered pilot pattern (clause 9.2.3, table 57) with its PILOT_PATTERN code (table 27).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PilotPattern {
    /// PP1: D_X 3, D_Y 4.
    Pp1,
    /// PP2: D_X 6, D_Y 2.
    Pp2,
    /// PP3: D_X 6, D_Y 4.
    Pp3,
    /// PP4: D_X 12, D_Y 2.
    Pp4,
    /// PP5: D_X 12, D_Y 4.
    Pp5,
    /// PP6: D_X 24, D_Y 2.
    Pp6,
    /// PP7: D_X 24, D_Y 4.
    Pp7,
    /// PP8: D_X 6, D_Y 16.
    Pp8,
}

impl PilotPattern {
    /// Every pattern, PP1 first.
    pub const ALL: [PilotPattern; 8] = [
        PilotPattern::Pp1,
        PilotPattern::Pp2,
        PilotPattern::Pp3,
        PilotPattern::Pp4,
        PilotPattern::Pp5,
        PilotPattern::Pp6,
        PilotPattern::Pp7,
        PilotPattern::Pp8,
    ];

    /// The PILOT_PATTERN code (0 for PP1).
    pub fn code(self) -> u8 {
        self as u8
    }

    /// The pattern of a PILOT_PATTERN code.
    pub fn from_code(c: u8) -> Option<PilotPattern> {
        PilotPattern::ALL.get(c as usize).copied()
    }

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }

    /// D_X, the separation of pilot-bearing carriers.
    pub fn dx(self) -> usize {
        [3, 6, 6, 12, 12, 24, 24, 6][self.code() as usize]
    }

    /// D_Y, the symbols of one scattered pilot sequence.
    pub fn dy(self) -> usize {
        [4, 2, 4, 2, 4, 2, 4, 16][self.code() as usize]
    }

    /// A_SP, the scattered pilot amplitude (table 60).
    pub fn a_sp(self) -> f32 {
        match self {
            PilotPattern::Pp1 | PilotPattern::Pp2 => 4.0 / 3.0,
            PilotPattern::Pp3 | PilotPattern::Pp4 => 7.0 / 4.0,
            _ => 7.0 / 3.0,
        }
    }
}

/// Whether a combination of FFT size, guard interval and pilot pattern is allowed (tables 58 and 59, and I.5 and I.6 for
/// T2-Lite).
pub fn allowed(fft: Fft, guard: Guard, pp: PilotPattern, miso: bool, lite: bool) -> bool {
    use Guard::*;
    use PilotPattern::*;
    if guard.samples(fft).is_none() {
        return false;
    }
    if lite {
        let list: &[PilotPattern] = match (miso, fft, guard) {
            (false, Fft::K16 | Fft::K8, G1_128) => &[Pp7],
            (false, Fft::K16, G1_32) => &[Pp7, Pp6],
            (false, Fft::K16, G1_16) => &[Pp4, Pp5],
            (false, Fft::K16, G19_256) => &[Pp2, Pp4, Pp5],
            (false, Fft::K8 | Fft::K4 | Fft::K2, G1_32) => &[Pp7, Pp4],
            (false, Fft::K8, G1_16 | G19_256) => &[Pp4, Pp5],
            (false, Fft::K4 | Fft::K2, G1_16) => &[Pp4, Pp5],
            (false, Fft::K16 | Fft::K8, G1_8 | G19_128) => &[Pp2, Pp3],
            (false, Fft::K4 | Fft::K2, G1_8) => &[Pp2, Pp3],
            (false, Fft::K16 | Fft::K8 | Fft::K4 | Fft::K2, G1_4) => &[Pp1],
            (true, Fft::K16 | Fft::K8, G1_128) => &[Pp4, Pp5],
            (true, Fft::K16, G1_32) => &[Pp4],
            (true, Fft::K8 | Fft::K4 | Fft::K2, G1_32) => &[Pp4, Pp5],
            (true, Fft::K16 | Fft::K8, G1_16 | G19_256) => &[Pp3],
            (true, Fft::K4 | Fft::K2, G1_16) => &[Pp3],
            (true, Fft::K16 | Fft::K8, G1_8 | G19_128) => &[Pp1],
            (true, Fft::K4 | Fft::K2, G1_8) => &[Pp1],
            _ => &[],
        };
        return list.contains(&pp);
    }
    let list: &[PilotPattern] = match (miso, fft, guard) {
        (false, Fft::K32, G1_128) => &[Pp7],
        (false, Fft::K32, G1_32) => &[Pp4, Pp6],
        (false, Fft::K32, G1_16 | G19_256) => &[Pp2, Pp8, Pp4],
        (false, Fft::K32, G1_8 | G19_128) => &[Pp2, Pp8],
        (false, Fft::K16, G1_128) => &[Pp7],
        (false, Fft::K16, G1_32) => &[Pp7, Pp4, Pp6],
        (false, Fft::K16, G1_16 | G19_256) => &[Pp2, Pp8, Pp4, Pp5],
        (false, Fft::K16, G1_8 | G19_128) => &[Pp2, Pp3, Pp8],
        (false, Fft::K16, G1_4) => &[Pp1, Pp8],
        (false, Fft::K8, G1_128) => &[Pp7],
        (false, Fft::K8, G1_32) => &[Pp7, Pp4],
        (false, Fft::K8, G1_16 | G19_256) => &[Pp8, Pp4, Pp5],
        (false, Fft::K8, G1_8 | G19_128) => &[Pp2, Pp3, Pp8],
        (false, Fft::K8, G1_4) => &[Pp1, Pp8],
        (false, Fft::K4 | Fft::K2, G1_32) => &[Pp7, Pp4],
        (false, Fft::K4 | Fft::K2 | Fft::K1, G1_16) => &[Pp4, Pp5],
        (false, Fft::K4 | Fft::K2 | Fft::K1, G1_8) => &[Pp2, Pp3],
        (false, Fft::K4 | Fft::K2 | Fft::K1, G1_4) => &[Pp1],
        (true, Fft::K32, G1_128) => &[Pp8, Pp4, Pp6],
        (true, Fft::K32, G1_32) => &[Pp8, Pp4],
        (true, Fft::K32, G1_16 | G19_256) => &[Pp2, Pp8],
        (true, Fft::K16 | Fft::K8, G1_128 | G1_32) => &[Pp8, Pp4, Pp5],
        (true, Fft::K16 | Fft::K8, G1_16 | G19_256) => &[Pp3, Pp8],
        (true, Fft::K16 | Fft::K8, G1_8 | G19_128) => &[Pp1, Pp8],
        (true, Fft::K4 | Fft::K2, G1_32) => &[Pp4, Pp5],
        (true, Fft::K4 | Fft::K2 | Fft::K1, G1_16) => &[Pp3],
        (true, Fft::K4 | Fft::K2 | Fft::K1, G1_8) => &[Pp1],
        _ => &[],
    };
    list.contains(&pp)
}

/// Whether the last symbol of a frame is a frame closing symbol (clause 9.2.7, table 64). In SISO mode a frame closing symbol
/// is used when D_X D_Y is at least the reciprocal of the guard fraction (the rule of note 4 of table 58, from which table 64
/// follows). In MISO mode it is always used except with PP8.
pub fn frame_closing(guard: Guard, pp: PilotPattern, miso: bool) -> bool {
    if pp == PilotPattern::Pp8 {
        return false;
    }
    if miso {
        return true;
    }
    let (n, d) = guard.fraction();
    pp.dx() * pp.dy() * n >= d
}

/// A constellation of the data and common PLPs (table 33), and BPSK for L1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Modulation {
    /// BPSK (L1 signalling only).
    Bpsk,
    /// QPSK.
    Qpsk,
    /// 16-QAM.
    Qam16,
    /// 64-QAM.
    Qam64,
    /// 256-QAM.
    Qam256,
}

impl Modulation {
    /// Bits per cell, eta_MOD.
    pub fn bits(self) -> usize {
        [1, 2, 4, 6, 8][self as usize]
    }

    /// The PLP_MOD code of table 33 (`None` for BPSK).
    pub fn plp_code(self) -> Option<u8> {
        (self != Modulation::Bpsk).then(|| self as u8 - 1)
    }

    /// The constellation of a PLP_MOD code.
    pub fn from_plp_code(c: u8) -> Option<Modulation> {
        [
            Modulation::Qpsk,
            Modulation::Qam16,
            Modulation::Qam64,
            Modulation::Qam256,
        ]
        .get(c as usize)
        .copied()
    }

    /// The L1_MOD code of table 24 (`None` for 256-QAM).
    pub fn l1_code(self) -> Option<u8> {
        (self != Modulation::Qam256).then_some(self as u8)
    }

    /// The constellation of an L1_MOD code.
    pub fn from_l1_code(c: u8) -> Option<Modulation> {
        [
            Modulation::Bpsk,
            Modulation::Qpsk,
            Modulation::Qam16,
            Modulation::Qam64,
        ]
        .get(c as usize)
        .copied()
    }

    /// The rotation angle in degrees (table 16).
    pub fn rotation_degrees(self) -> f64 {
        match self {
            Modulation::Bpsk => 0.0,
            Modulation::Qpsk => 29.0,
            Modulation::Qam16 => 16.8,
            Modulation::Qam64 => 8.6,
            Modulation::Qam256 => (1.0f64 / 16.0).atan().to_degrees(),
        }
    }

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }
}

/// An LDPC code identifier (tables 6(a), 6(b) and I.1). 1/4 protects L1-pre only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CodeRate {
    /// 1/2.
    R1_2,
    /// 3/5.
    R3_5,
    /// 2/3.
    R2_3,
    /// 3/4.
    R3_4,
    /// 4/5 (T2-base only).
    R4_5,
    /// 5/6 (T2-base only).
    R5_6,
    /// 1/3 (T2-Lite only).
    R1_3,
    /// 2/5 (T2-Lite only).
    R2_5,
    /// 1/4 (L1-pre only).
    R1_4,
}

impl CodeRate {
    /// The PLP_COD code of table 32.
    pub fn code(self) -> Option<u8> {
        match self {
            CodeRate::R1_4 => None,
            CodeRate::R1_3 => Some(6),
            CodeRate::R2_5 => Some(7),
            r => Some(r as u8),
        }
    }

    /// The code rate of a PLP_COD code for a profile.
    pub fn from_code(c: u8, lite: bool) -> Option<CodeRate> {
        match (c, lite) {
            (0, _) => Some(CodeRate::R1_2),
            (1, _) => Some(CodeRate::R3_5),
            (2, _) => Some(CodeRate::R2_3),
            (3, _) => Some(CodeRate::R3_4),
            (4, false) => Some(CodeRate::R4_5),
            (5, false) => Some(CodeRate::R5_6),
            (6, true) => Some(CodeRate::R1_3),
            (7, true) => Some(CodeRate::R2_5),
            _ => None,
        }
    }

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }
}

/// The FEC frame length (PLP_FEC_TYPE, table 34).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FecFrame {
    /// 16 200 bits (code 00).
    Short,
    /// 64 800 bits (code 01).
    Normal,
}

impl FecFrame {
    /// N_ldpc.
    pub fn n_ldpc(self) -> usize {
        match self {
            FecFrame::Short => 16_200,
            FecFrame::Normal => 64_800,
        }
    }

    /// The PLP_FEC_TYPE code.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// The frame of a PLP_FEC_TYPE code.
    pub fn from_code(c: u8) -> Option<FecFrame> {
        match c {
            0 => Some(FecFrame::Short),
            1 => Some(FecFrame::Normal),
            _ => None,
        }
    }
}

/// The LDPC address table of a frame and code rate (annexes A and B, with the DVB-S2 tables where the standard repeats them). `None`
/// for a pair the standard does not define.
pub fn ldpc_table(frame: FecFrame, rate: CodeRate) -> Option<&'static DvbTable> {
    use CodeRate::*;
    Some(match (frame, rate) {
        (FecFrame::Normal, R1_2) => &dvb_s2::S2_N_1_2,
        (FecFrame::Normal, R3_5) => &dvb_s2::S2_N_3_5,
        (FecFrame::Normal, R2_3) => &dvb_t2::T2_N_2_3,
        (FecFrame::Normal, R3_4) => &dvb_s2::S2_N_3_4,
        (FecFrame::Normal, R4_5) => &dvb_s2::S2_N_4_5,
        (FecFrame::Normal, R5_6) => &dvb_s2::S2_N_5_6,
        (FecFrame::Short, R1_4) => &dvb_s2::S2_S_1_4,
        (FecFrame::Short, R1_3) => &dvb_s2::S2_S_1_3,
        (FecFrame::Short, R2_5) => &dvb_s2::S2_S_2_5,
        (FecFrame::Short, R1_2) => &dvb_s2::S2_S_1_2,
        (FecFrame::Short, R3_5) => &dvb_t2::T2_S_3_5,
        (FecFrame::Short, R2_3) => &dvb_s2::S2_S_2_3,
        (FecFrame::Short, R3_4) => &dvb_s2::S2_S_3_4,
        (FecFrame::Short, R4_5) => &dvb_s2::S2_S_4_5,
        (FecFrame::Short, R5_6) => &dvb_s2::S2_S_5_6,
        _ => return None,
    })
}

/// The key of the BCH code in `sdrkit_algo_fec::bch::codes` for a frame and code rate. DVB-T2 tables 6 and 7 equal the DVB-S2 ones.
pub fn outer_code_key(frame: FecFrame, rate: CodeRate) -> Option<&'static str> {
    use CodeRate::*;
    Some(match (frame, rate) {
        (FecFrame::Normal, R1_2) => "dvb_s2_normal_1_2",
        (FecFrame::Normal, R3_5) => "dvb_s2_normal_3_5",
        (FecFrame::Normal, R2_3) => "dvb_s2_normal_2_3",
        (FecFrame::Normal, R3_4) => "dvb_s2_normal_3_4",
        (FecFrame::Normal, R4_5) => "dvb_s2_normal_4_5",
        (FecFrame::Normal, R5_6) => "dvb_s2_normal_5_6",
        (FecFrame::Short, R1_4) => "dvb_s2_short_1_4",
        (FecFrame::Short, R1_3) => "dvb_s2_short_1_3",
        (FecFrame::Short, R2_5) => "dvb_s2_short_2_5",
        (FecFrame::Short, R1_2) => "dvb_s2_short_1_2",
        (FecFrame::Short, R3_5) => "dvb_s2_short_3_5",
        (FecFrame::Short, R2_3) => "dvb_s2_short_2_3",
        (FecFrame::Short, R3_4) => "dvb_s2_short_3_4",
        (FecFrame::Short, R4_5) => "dvb_s2_short_4_5",
        (FecFrame::Short, R5_6) => "dvb_s2_short_5_6",
        _ => return None,
    })
}

/// K_bch, the BBFRAME length in bits (tables 6(a), 6(b) and I.1).
pub fn k_bch(frame: FecFrame, rate: CodeRate) -> Option<usize> {
    use CodeRate::*;
    Some(match (frame, rate) {
        (FecFrame::Normal, R1_2) => 32_208,
        (FecFrame::Normal, R3_5) => 38_688,
        (FecFrame::Normal, R2_3) => 43_040,
        (FecFrame::Normal, R3_4) => 48_408,
        (FecFrame::Normal, R4_5) => 51_648,
        (FecFrame::Normal, R5_6) => 53_840,
        (FecFrame::Short, R1_4) => 3_072,
        (FecFrame::Short, R1_3) => 5_232,
        (FecFrame::Short, R2_5) => 6_312,
        (FecFrame::Short, R1_2) => 7_032,
        (FecFrame::Short, R3_5) => 9_552,
        (FecFrame::Short, R2_3) => 10_632,
        (FecFrame::Short, R3_4) => 11_712,
        (FecFrame::Short, R4_5) => 12_432,
        (FecFrame::Short, R5_6) => 13_152,
        _ => return None,
    })
}

/// N_cells, the cells of one FEC block (table 17).
pub fn cells_per_block(frame: FecFrame, m: Modulation) -> usize {
    frame.n_ldpc() / m.bits()
}

/// The channel width, with the elementary period T of table 65.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bandwidth {
    /// 1.7 MHz: T = 71/131 µs.
    B1_7,
    /// 5 MHz: T = 7/40 µs.
    B5,
    /// 6 MHz: T = 7/48 µs.
    B6,
    /// 7 MHz: T = 1/8 µs.
    B7,
    /// 8 MHz: T = 7/64 µs.
    B8,
    /// 10 MHz: T = 7/80 µs.
    B10,
}

impl Bandwidth {
    /// Every width.
    pub const ALL: [Bandwidth; 6] = [
        Bandwidth::B1_7,
        Bandwidth::B5,
        Bandwidth::B6,
        Bandwidth::B7,
        Bandwidth::B8,
        Bandwidth::B10,
    ];

    /// The label for people ("8K", "1/8", "PP7", "64-QAM", "2/3", "8 MHz").
    pub fn name(self) -> &'static str {
        Choice::label(self)
    }

    /// The nominal channel width in Hz.
    pub fn hz(self) -> f64 {
        [1.712e6, 5e6, 6e6, 7e6, 8e6, 10e6][self as usize]
    }

    /// The sample rate 1/T in Hz (the natural rate of the signal).
    pub fn rate(self) -> f64 {
        match self {
            Bandwidth::B1_7 => 131e6 / 71.0,
            Bandwidth::B5 => 40e6 / 7.0,
            Bandwidth::B6 => 48e6 / 7.0,
            Bandwidth::B7 => 8e6,
            Bandwidth::B8 => 64e6 / 7.0,
            Bandwidth::B10 => 80e6 / 7.0,
        }
    }
}

/// The profile: T2-base or T2-Lite (annex I).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Profile {
    /// T2-base.
    Base,
    /// T2-Lite.
    Lite,
}

/// What P1 signals (clause 7.2.1, tables 18 and 19(a)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct P1Info {
    /// The S1 field (3 bits).
    pub s1: u8,
    /// The S2 field (4 bits): S2 field 1 in the top 3 bits, the "mixed" bit last.
    pub s2: u8,
}

impl P1Info {
    /// The P1 fields of a T2 frame.
    pub fn new(
        fft: Fft,
        guard: Guard,
        profile: Profile,
        miso: bool,
        mixed: bool,
    ) -> Option<P1Info> {
        let s1 = match (profile, miso) {
            (Profile::Base, false) => 0,
            (Profile::Base, true) => 1,
            (Profile::Lite, false) => 3,
            (Profile::Lite, true) => 4,
        };
        let short = guard.is_short_family();
        let f1 = match (profile, fft) {
            (_, Fft::K2) => 0,
            (_, Fft::K8) if short => 6,
            (_, Fft::K8) => 1,
            (_, Fft::K4) => 2,
            (Profile::Base, Fft::K1) => 3,
            (Profile::Base, Fft::K16) => 4,
            (Profile::Base, Fft::K32) if short => 7,
            (Profile::Base, Fft::K32) => 5,
            (Profile::Lite, Fft::K16) if short => 3,
            (Profile::Lite, Fft::K16) => 4,
            _ => return None,
        };
        Some(P1Info {
            s1,
            s2: (f1 << 1) | mixed as u8,
        })
    }

    /// Whether this P1 starts a T2 frame (S1 is T2_SISO, T2_MISO, T2_LITE_SISO or T2_LITE_MISO).
    pub fn is_t2(self) -> bool {
        matches!(self.s1, 0 | 1 | 3 | 4)
    }

    /// The profile, for a T2 preamble.
    pub fn profile(self) -> Option<Profile> {
        match self.s1 {
            0 | 1 => Some(Profile::Base),
            3 | 4 => Some(Profile::Lite),
            _ => None,
        }
    }

    /// Whether P2 is in its MISO format.
    pub fn miso(self) -> bool {
        matches!(self.s1, 1 | 4)
    }

    /// The FFT size, and the guard intervals it allows, from S2 field 1.
    pub fn fft(self) -> Option<(Fft, &'static [Guard])> {
        use Guard::*;
        const LONG: &[Guard] = &[G1_32, G1_16, G1_8, G1_4];
        const SHORT: &[Guard] = &[G1_128, G19_256, G19_128];
        const ANY: &[Guard] = &[G1_128, G1_32, G1_16, G19_256, G1_8, G19_128, G1_4];
        let lite = self.profile()? == Profile::Lite;
        Some(match (self.s2 >> 1, lite) {
            (0, _) => (Fft::K2, ANY),
            (1, _) => (Fft::K8, LONG),
            (2, _) => (Fft::K4, ANY),
            (3, false) => (Fft::K1, ANY),
            (3, true) => (Fft::K16, SHORT),
            (4, false) => (Fft::K16, ANY),
            (4, true) => (Fft::K16, LONG),
            (5, false) => (Fft::K32, &[G1_32, G1_16, G1_8]),
            (6, _) => (Fft::K8, SHORT),
            (7, false) => (Fft::K32, SHORT),
            _ => return None,
        })
    }
}
