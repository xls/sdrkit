//! DRM robustness modes, spectrum occupancies, the frame layout (pilot, FAC, SDC and MSC cells), code rates and interleavers.
//!
//! Standard: ETSI ES 201 980 V4.3.1 clauses 7 and 8.

use super::mlc::Rate;
use sdrkit_algo_modem::ofdm::FrameLayout;
use sdrkit_core::Complex32;

sdrkit_core::choice_enum! {
    /// A square constellation of DRM with its set-partitioned mapping (clause 7.4, figures 26, 29 and 30).
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum Qam {
        /// 4-QAM.
        Q4 = ("4", "4-QAM"),
        /// 16-QAM.
        Q16 = ("16", "16-QAM"),
        /// 64-QAM.
        Q64 = ("64", "64-QAM"),
    }
}

impl Qam {
    /// Bits per axis (the levels of the multilevel code).
    pub fn bits_per_axis(self) -> usize {
        match self {
            Qam::Q4 => 1,
            Qam::Q16 => 2,
            Qam::Q64 => 3,
        }
    }
    /// Bits per cell.
    pub fn bits(self) -> usize {
        2 * self.bits_per_axis()
    }
    /// Points per axis.
    pub fn levels(self) -> usize {
        1 << self.bits_per_axis()
    }
    /// The normalisation `a` of clause 7.4: 1/sqrt(2), 1/sqrt(10) and 1/sqrt(42), for a mean power of 1.
    pub fn scale(self) -> f32 {
        match self {
            Qam::Q4 => std::f32::consts::FRAC_1_SQRT_2,
            Qam::Q16 => 1.0 / 10f32.sqrt(),
            Qam::Q64 => 1.0 / 42f32.sqrt(),
        }
    }
    /// A name ("4-QAM").
    pub fn name(self) -> &'static str {
        match self {
            Qam::Q4 => "4-QAM",
            Qam::Q16 => "16-QAM",
            Qam::Q64 => "64-QAM",
        }
    }
    /// The axis value of the bits `idx = b0 + 2 b1 + 4 b2` of one axis: `(L - 1 - 2 idx) a`, so all zeros is the largest
    /// positive value (clause 7.4, figures 26, 29 and 30).
    #[inline]
    pub fn axis(self, idx: usize) -> f32 {
        (self.levels() as f32 - 1.0 - 2.0 * idx as f32) * self.scale()
    }
    /// The cell of the in-phase bits `i` and the quadrature bits `q` (each `b0 + 2 b1 + 4 b2`).
    #[inline]
    pub fn map(self, i: usize, q: usize) -> Complex32 {
        Complex32::new(self.axis(i), self.axis(q))
    }
    /// The nearest point (for error measurements).
    pub fn nearest(self, y: Complex32) -> Complex32 {
        let s = self.scale();
        let l = self.levels() as f32;
        let snap = |v: f32| {
            let idx = ((v / s + (l - 1.0)) / 2.0).round().clamp(0.0, l - 1.0);
            (2.0 * idx - (l - 1.0)) * s
        };
        Complex32::new(snap(y.re), snap(y.im))
    }
}

sdrkit_core::choice_enum! {
    /// A robustness mode.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub enum Robustness {
        /// Mode A.
        A = ("A", "A"),
        /// Mode B.
        B = ("B", "B"),
        /// Mode C.
        C = ("C", "C"),
        /// Mode D.
        D = ("D", "D"),
        /// Mode E (DRM+).
        E = ("E", "E"),
    }
}

impl Robustness {
    /// All modes.
    pub const ALL: [Robustness; 5] = [
        Robustness::A,
        Robustness::B,
        Robustness::C,
        Robustness::D,
        Robustness::E,
    ];
    /// "A" .. "E".
    pub fn letter(self) -> &'static str {
        ["A", "B", "C", "D", "E"][self as usize]
    }
    /// From a letter.
    pub fn from_letter(s: &str) -> Option<Robustness> {
        Some(match s {
            "A" | "a" => Robustness::A,
            "B" | "b" => Robustness::B,
            "C" | "c" => Robustness::C,
            "D" | "d" => Robustness::D,
            "E" | "e" => Robustness::E,
            _ => return None,
        })
    }
    /// The OFDM rate the mode is demodulated at.
    pub fn rate(self) -> f64 {
        if self == Robustness::E {
            192_000.0
        } else {
            48_000.0
        }
    }
    /// Useful part, samples at [`rate`](Self::rate).
    pub fn nfft(self) -> usize {
        [1152, 1024, 704, 448, 432][self as usize]
    }
    /// Guard interval, samples.
    pub fn cp(self) -> usize {
        [128, 256, 256, 352, 48][self as usize]
    }
    /// Symbols per frame.
    pub fn nsym(self) -> usize {
        [15, 15, 20, 24, 40][self as usize]
    }
    /// Frames per super frame.
    pub fn frames_per_superframe(self) -> usize {
        if self == Robustness::E {
            4
        } else {
            3
        }
    }
    /// Frame duration, seconds.
    pub fn frame_s(self) -> f64 {
        (self.nsym() * (self.nfft() + self.cp())) as f64 / self.rate()
    }
    /// Carrier spacing, Hz.
    pub fn spacing(self) -> f64 {
        self.rate() / self.nfft() as f64
    }
    /// Long interleaver depth in frames.
    pub fn long_depth(self) -> usize {
        if self == Robustness::E {
            6
        } else {
            5
        }
    }
    /// The union of the carrier ranges of every occupancy.
    pub fn union_range(self) -> (i32, i32) {
        [
            (-114, 350),
            (-103, 311),
            (-69, 213),
            (-44, 135),
            (-106, 106),
        ][self as usize]
    }
    /// The carriers (kmin, kmax) of an occupancy (table 49, clause 8.3.1), `None` when the mode does not allow it.
    pub fn range(self, occ: u8) -> Option<(i32, i32)> {
        let r = match self {
            Robustness::A => [
                Some((2, 102)),
                Some((2, 114)),
                Some((-102, 102)),
                Some((-114, 114)),
                Some((-98, 314)),
                Some((-110, 350)),
            ],
            Robustness::B => [
                Some((1, 91)),
                Some((1, 103)),
                Some((-91, 91)),
                Some((-103, 103)),
                Some((-87, 279)),
                Some((-99, 311)),
            ],
            Robustness::C => [None, None, None, Some((-69, 69)), None, Some((-67, 213))],
            Robustness::D => [None, None, None, Some((-44, 44)), None, Some((-43, 135))],
            Robustness::E => [Some((-106, 106)), None, None, None, None, None],
        };
        r.get(occ as usize).copied().flatten()
    }
    /// The usual occupancy: 10 kHz, mode E 100 kHz.
    pub fn usual_occupancy(self) -> u8 {
        if self == Robustness::E {
            0
        } else {
            3
        }
    }
    /// Whether carrier `k` is an unused carrier of table 50 (clause 8.3.1): A -1, 0 and 1, B to D 0, E none.
    pub fn unused(self, k: i32) -> bool {
        match self {
            Robustness::A => (-1..=1).contains(&k),
            Robustness::E => false,
            _ => k == 0,
        }
    }
    /// FAC cells per frame (clause 7.2.1.2): 65, mode E 244.
    pub fn fac_cells(self) -> usize {
        if self == Robustness::E {
            244
        } else {
            65
        }
    }
    /// FAC information bits including the CRC (clause 7.2.1.2): 72, mode E 116.
    pub fn fac_bits(self) -> usize {
        if self == Robustness::E {
            116
        } else {
            72
        }
    }
    /// The SDC symbols: symbols 0 to `sdc_symbols() - 1` of the first frame of a super frame (clause 8.5.3.1).
    pub fn sdc_symbols(self) -> usize {
        SDC_SYMBOLS[self as usize]
    }
}

/// Occupancy bandwidth in kHz (table 48).
pub fn occupancy_khz(r: Robustness, occ: u8) -> f64 {
    if r == Robustness::E {
        100.0
    } else {
        [4.5, 5.0, 9.0, 10.0, 18.0, 20.0][occ as usize % 6]
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Pilot cells (clause 8.4). Phases are phase indices in 1/1024 of a cycle.

/// The frequency pilots of each mode: carrier and phase index in symbol 0 (ES 201 980 clause 8.4.2, tables 51 and 52).
/// Mode E has none.
pub const FREQ_PILOTS: [&[(i32, u16)]; 5] = [
    &[(18, 205), (54, 836), (72, 215)],
    &[(16, 331), (48, 651), (64, 555)],
    &[(11, 214), (33, 392), (44, 242)],
    &[(7, 788), (21, 1014), (28, 332)],
    &[],
];

/// The time pilots of mode A: carrier and phase index (ES 201 980 clause 8.4.3.1, table 53).
pub const TIME_PILOTS_A: [(i32, u16); 21] = [
    (17, 973),
    (18, 205),
    (19, 717),
    (21, 264),
    (28, 357),
    (29, 357),
    (32, 952),
    (33, 440),
    (39, 856),
    (40, 88),
    (41, 88),
    (53, 68),
    (54, 836),
    (55, 836),
    (56, 836),
    (60, 1008),
    (61, 1008),
    (63, 752),
    (71, 215),
    (72, 215),
    (73, 727),
];

/// The time pilots of mode B: carrier and phase index (ES 201 980 clause 8.4.3.1, table 54).
pub const TIME_PILOTS_B: [(i32, u16); 19] = [
    (14, 304),
    (16, 331),
    (18, 108),
    (20, 620),
    (24, 192),
    (26, 704),
    (32, 44),
    (36, 432),
    (42, 588),
    (44, 844),
    (48, 651),
    (49, 651),
    (50, 651),
    (54, 460),
    (56, 460),
    (62, 944),
    (64, 555),
    (66, 940),
    (68, 428),
];

/// The time pilots of mode C: carrier and phase index (ES 201 980 clause 8.4.3.1, table 55).
pub const TIME_PILOTS_C: [(i32, u16); 19] = [
    (8, 722),
    (10, 466),
    (11, 214),
    (12, 214),
    (14, 479),
    (16, 516),
    (18, 260),
    (22, 577),
    (24, 662),
    (28, 3),
    (30, 771),
    (32, 392),
    (33, 392),
    (36, 37),
    (38, 37),
    (42, 474),
    (44, 242),
    (45, 242),
    (46, 754),
];

/// The time pilots of mode D: carrier and phase index (ES 201 980 clause 8.4.3.1, table 56).
pub const TIME_PILOTS_D: [(i32, u16); 21] = [
    (5, 636),
    (6, 124),
    (7, 788),
    (8, 788),
    (9, 200),
    (11, 688),
    (12, 152),
    (14, 920),
    (15, 920),
    (17, 644),
    (18, 388),
    (20, 652),
    (21, 1014),
    (23, 176),
    (24, 176),
    (26, 752),
    (27, 496),
    (28, 332),
    (29, 432),
    (30, 964),
    (32, 452),
];

/// The time pilots of mode E: carrier and phase index (ES 201 980 clause 8.4.3.1, table 57).
pub const TIME_PILOTS_E: [(i32, u16); 21] = [
    (-80, 219),
    (-79, 475),
    (-77, 987),
    (-53, 652),
    (-52, 652),
    (-51, 140),
    (-32, 819),
    (-31, 819),
    (12, 907),
    (13, 907),
    (14, 651),
    (21, 903),
    (22, 391),
    (23, 903),
    (40, 203),
    (41, 203),
    (42, 203),
    (67, 797),
    (68, 29),
    (79, 508),
    (80, 508),
];

/// The time pilots of a mode (tables 53 to 57).
pub fn time_pilots(r: Robustness) -> &'static [(i32, u16)] {
    match r {
        Robustness::A => &TIME_PILOTS_A,
        Robustness::B => &TIME_PILOTS_B,
        Robustness::C => &TIME_PILOTS_C,
        Robustness::D => &TIME_PILOTS_D,
        Robustness::E => &TIME_PILOTS_E,
    }
}

/// The gain pilot pattern (x, y, k0) of each mode (ES 201 980 clauses 8.4.4.1 and 8.4.4.3.1, tables 58 and 60): symbol `s`
/// has gain pilots on the carriers `k0 + x (s mod y) + x y p` for every integer `p`.
pub const GAIN_PATTERN: [(i32, i32, i32); 5] =
    [(4, 5, 2), (2, 3, 1), (2, 2, 1), (1, 3, 1), (4, 4, 2)];

/// Mode A `W1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.2).
pub const GAIN_W_A: [[u16; 3]; 5] = [
    [228, 341, 455],
    [455, 569, 683],
    [683, 796, 910],
    [910, 0, 114],
    [114, 228, 341],
];
/// Mode A `Z256[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.2).
pub const GAIN_Z_A: [[u16; 3]; 5] = [
    [0, 81, 248],
    [18, 106, 106],
    [122, 116, 31],
    [129, 129, 39],
    [33, 32, 111],
];
/// Mode A `Q1024` of the gain pilot phases (ES 201 980 clause 8.4.4.3.2).
pub const GAIN_Q_A: u16 = 36;

/// Mode B `W1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.3).
pub const GAIN_W_B: [[u16; 5]; 3] = [
    [512, 0, 512, 0, 512],
    [0, 512, 0, 512, 0],
    [512, 0, 512, 0, 512],
];
/// Mode B `Z256[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.3).
pub const GAIN_Z_B: [[u16; 5]; 3] = [
    [0, 57, 164, 64, 12],
    [168, 255, 161, 106, 118],
    [25, 232, 132, 233, 38],
];
/// Mode B `Q1024` of the gain pilot phases (ES 201 980 clause 8.4.4.3.3).
pub const GAIN_Q_B: u16 = 12;

/// Mode C `W1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.4).
pub const GAIN_W_C: [[u16; 10]; 2] = [
    [465, 372, 279, 186, 93, 0, 931, 838, 745, 652],
    [931, 838, 745, 652, 559, 465, 372, 279, 186, 93],
];
/// Mode C `Z256[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.4).
pub const GAIN_Z_C: [[u16; 10]; 2] = [
    [0, 76, 29, 76, 9, 190, 161, 248, 33, 108],
    [179, 178, 83, 253, 127, 105, 101, 198, 250, 145],
];
/// Mode C `Q1024` of the gain pilot phases (ES 201 980 clause 8.4.4.3.4).
pub const GAIN_Q_C: u16 = 12;

/// Mode D `W1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.5).
pub const GAIN_W_D: [[u16; 8]; 3] = [
    [366, 439, 512, 585, 658, 731, 805, 878],
    [731, 805, 878, 951, 0, 73, 146, 219],
    [73, 146, 219, 293, 366, 439, 512, 585],
];
/// Mode D `Z256[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.5).
pub const GAIN_Z_D: [[u16; 8]; 3] = [
    [0, 240, 17, 60, 220, 38, 151, 101],
    [110, 7, 78, 82, 175, 150, 106, 25],
    [165, 7, 252, 124, 253, 177, 197, 142],
];
/// Mode D `Q1024` of the gain pilot phases (ES 201 980 clause 8.4.4.3.5).
pub const GAIN_Q_D: u16 = 14;

/// Mode E `R1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.6).
pub const GAIN_R_E: [[u16; 10]; 4] = [
    [39, 118, 197, 276, 354, 433, 39, 118, 197, 276],
    [37, 183, 402, 37, 183, 402, 37, 183, 402, 37],
    [110, 329, 475, 110, 329, 475, 110, 329, 475, 110],
    [79, 158, 236, 315, 394, 473, 79, 158, 236, 315],
];
/// Mode E `Z1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.6).
pub const GAIN_Z_E: [[u16; 10]; 4] = [
    [473, 394, 315, 236, 158, 79, 0, 0, 0, 0],
    [183, 914, 402, 37, 475, 841, 768, 768, 987, 183],
    [549, 622, 475, 110, 37, 622, 256, 768, 329, 549],
    [79, 158, 236, 315, 394, 473, 158, 315, 473, 630],
];
/// Mode E `Q1024[n, m]` of the gain pilot phases (ES 201 980 clause 8.4.4.3.6).
pub const GAIN_Q_E: [[u16; 10]; 4] = [
    [329, 489, 894, 419, 607, 519, 1020, 942, 817, 939],
    [824, 1023, 74, 319, 225, 207, 348, 422, 395, 92],
    [959, 379, 7, 738, 500, 920, 440, 727, 263, 733],
    [907, 946, 924, 91, 189, 133, 910, 804, 1022, 433],
];

/// The gain pilots with a power gain of 4 (amplitude 2) of each mode and occupancy (ES 201 980 clause 8.4.4.2, table 59).
pub const BOOSTED_PILOTS: [[Option<[i32; 4]>; 6]; 5] = [
    [
        Some([2, 6, 98, 102]),
        Some([2, 6, 110, 114]),
        Some([-102, -98, 98, 102]),
        Some([-114, -110, 110, 114]),
        Some([-98, -94, 310, 314]),
        Some([-110, -106, 346, 350]),
    ],
    [
        Some([1, 3, 89, 91]),
        Some([1, 3, 101, 103]),
        Some([-91, -89, 89, 91]),
        Some([-103, -101, 101, 103]),
        Some([-87, -85, 277, 279]),
        Some([-99, -97, 309, 311]),
    ],
    [
        None,
        None,
        None,
        Some([-69, -67, 67, 69]),
        None,
        Some([-67, -65, 211, 213]),
    ],
    [
        None,
        None,
        None,
        Some([-44, -43, 43, 44]),
        None,
        Some([-43, -42, 134, 135]),
    ],
    [Some([-106, -102, 102, 106]), None, None, None, None, None],
];

/// The AFS pilots of mode E: carrier, phase index in symbol 4 of the first frame, phase index in symbol 39 of the fourth frame
/// (ES 201 980 clause 8.4.5.1, table 61). Where an AFS pilot is also a gain pilot, the gain pilot's amplitude applies.
pub const AFS_PILOTS: [(i32, u16, u16); 54] = [
    (-106, 134, 115),
    (-102, 866, 135),
    (-98, 588, 194),
    (-94, 325, 293),
    (-90, 77, 431),
    (-86, 868, 608),
    (-82, 649, 825),
    (-78, 445, 57),
    (-74, 256, 353),
    (-70, 82, 688),
    (-66, 946, 38),
    (-62, 801, 452),
    (-58, 671, 905),
    (-54, 556, 373),
    (-50, 455, 905),
    (-46, 369, 452),
    (-42, 298, 39),
    (-38, 242, 689),
    (-34, 200, 354),
    (-30, 173, 59),
    (-26, 161, 827),
    (-22, 164, 610),
    (-18, 181, 433),
    (-14, 213, 295),
    (-10, 260, 197),
    (-6, 322, 138),
    (-2, 398, 118),
    (2, 489, 138),
    (6, 595, 197),
    (10, 716, 295),
    (14, 851, 433),
    (18, 1001, 610),
    (22, 142, 827),
    (26, 322, 59),
    (30, 516, 354),
    (34, 725, 689),
    (38, 949, 39),
    (42, 164, 452),
    (46, 417, 905),
    (50, 685, 373),
    (54, 968, 905),
    (58, 242, 452),
    (62, 554, 38),
    (66, 881, 688),
    (70, 199, 353),
    (74, 556, 57),
    (78, 927, 825),
    (82, 289, 608),
    (86, 690, 431),
    (90, 82, 293),
    (94, 512, 194),
    (98, 957, 135),
    (102, 393, 115),
    (106, 868, 134),
];

/// The (frame of the super frame, symbol) pairs that hold the AFS pilots of mode E (clause 8.4.5.0).
pub const AFS_SYMBOLS: [(usize, usize); 2] = [(0, 4), (3, 39)];

/// The cell value `a e^(j 2 pi theta / 1024)` of a pilot with amplitude `a` and phase index `theta` (clause 8.4.1).
pub fn pilot_value(a: f32, theta: u16) -> Complex32 {
    let ph = std::f64::consts::TAU * theta as f64 / 1024.0;
    Complex32::new((a as f64 * ph.cos()) as f32, (a as f64 * ph.sin()) as f32)
}

/// The phase index of the frequency pilot on carrier `k` in symbol `s` (clause 8.4.2.2), `None` when `k` has none. The phase
/// of symbol 0 holds in every symbol, except on carriers 7 and 21 of mode D, which add 512 in the odd symbols.
pub fn freq_pilot(r: Robustness, s: usize, k: i32) -> Option<u16> {
    let &(_, theta) = FREQ_PILOTS[r as usize].iter().find(|p| p.0 == k)?;
    let flip = r == Robustness::D && (k == 7 || k == 21) && s % 2 == 1;
    Some(if flip { (theta + 512) % 1024 } else { theta })
}

/// The phase index of the time pilot on carrier `k` (symbol 0 only, clause 8.4.3.1), `None` when `k` has none.
pub fn time_pilot(r: Robustness, k: i32) -> Option<u16> {
    time_pilots(r).iter().find(|p| p.0 == k).map(|p| p.1)
}

/// The phase index of the gain pilot on carrier `k` in symbol `s` (clause 8.4.4.3.1), `None` when the pattern of table 58
/// has no gain pilot there. With `n = s mod y`, `m = s / y` and `p = (k - k0 - x n) / (x y)`, modes A to D use
/// `(4 Z256[n, m] + p W1024[n, m] + p^2 (1 + s) Q1024) mod 1024` and mode E `(p^2 R1024[n, m] + p Z1024[n, m] + Q1024[n, m])
/// mod 1024`. The occupancy range is not checked here.
pub fn gain_phase(r: Robustness, s: usize, k: i32) -> Option<u16> {
    let (x, y, k0) = GAIN_PATTERN[r as usize];
    let si = s as i64;
    let n = (si % y as i64) as usize;
    let m = (si / y as i64) as usize;
    let off = (k - k0 - x * n as i32) as i64;
    let xy = (x * y) as i64;
    if off.rem_euclid(xy) != 0 {
        return None;
    }
    let p = off.div_euclid(xy);
    let ad = |w: u16, z: u16, q: u16| 4 * z as i64 + p * w as i64 + p * p * (1 + si) * q as i64;
    let v = match r {
        Robustness::A => ad(*GAIN_W_A.get(n)?.get(m)?, GAIN_Z_A[n][m], GAIN_Q_A),
        Robustness::B => ad(*GAIN_W_B.get(n)?.get(m)?, GAIN_Z_B[n][m], GAIN_Q_B),
        Robustness::C => ad(*GAIN_W_C.get(n)?.get(m)?, GAIN_Z_C[n][m], GAIN_Q_C),
        Robustness::D => ad(*GAIN_W_D.get(n)?.get(m)?, GAIN_Z_D[n][m], GAIN_Q_D),
        Robustness::E => {
            let rr = *GAIN_R_E.get(n)?.get(m)? as i64;
            p * p * rr + p * GAIN_Z_E[n][m] as i64 + GAIN_Q_E[n][m] as i64
        }
    };
    Some(v.rem_euclid(1024) as u16)
}

/// Whether carrier `k` holds a gain pilot with a power gain of 4 in occupancy `occ` (table 59).
pub fn is_boosted(r: Robustness, occ: u8, k: i32) -> bool {
    BOOSTED_PILOTS[r as usize]
        .get(occ as usize)
        .copied()
        .flatten()
        .is_some_and(|b| b.contains(&k))
}

/// The value of cell (s, k) in mode `r` and occupancy `occ` when it is a time, frequency or gain pilot (clause 8.4), else
/// `None`. Carriers outside the occupancy and the unused carriers of table 50 have no pilot. Where the patterns meet, the
/// time pilot phase comes first, then the frequency pilot phase (clause 8.4.4.3.0). Every pilot has a power gain of 2 except
/// the gain pilots of table 59, which have 4.
pub fn pilot(r: Robustness, occ: u8, s: usize, k: i32) -> Option<Complex32> {
    let (lo, hi) = r.range(occ)?;
    if k < lo || k > hi || r.unused(k) {
        return None;
    }
    let sqrt2 = std::f32::consts::SQRT_2;
    if s == 0 {
        if let Some(t) = time_pilot(r, k) {
            return Some(pilot_value(sqrt2, t));
        }
    }
    if let Some(t) = freq_pilot(r, s, k) {
        return Some(pilot_value(sqrt2, t));
    }
    let t = gain_phase(r, s, k)?;
    let a = if is_boosted(r, occ, k) { 2.0 } else { sqrt2 };
    Some(pilot_value(a, t))
}

/// The value of an AFS pilot of mode E in symbol `s` of frame `frame` of the super frame, when it is not also a gain pilot
/// (clause 8.4.5): amplitude 1 and the phase of table 61. `None` for every other cell and mode.
pub fn afs_pilot(r: Robustness, frame: usize, s: usize, k: i32) -> Option<Complex32> {
    if r != Robustness::E || gain_phase(r, s, k).is_some() {
        return None;
    }
    let col = AFS_SYMBOLS.iter().position(|&a| a == (frame, s))?;
    let &(_, t4, t39) = AFS_PILOTS.iter().find(|p| p.0 == k)?;
    Some(pilot_value(1.0, if col == 0 { t4 } else { t39 }))
}

/// The value of cell (s, k) of frame `frame` of the super frame when it is any pilot: [`pilot`] or [`afs_pilot`].
pub fn frame_pilot(r: Robustness, occ: u8, frame: usize, s: usize, k: i32) -> Option<Complex32> {
    pilot(r, occ, s, k).or_else(|| afs_pilot(r, frame, s, k))
}

/// The OFDM layout of mode `r` and occupancy `occ` over the union carrier range of the mode. The known cells are the time,
/// frequency and gain pilots of the occupancy (the same in every frame). The AFS pilots of mode E are not in it, because they
/// are in two frames of the super frame only. `None` when the mode does not allow the occupancy.
pub fn layout(r: Robustness, occ: u8) -> Option<FrameLayout> {
    r.range(occ)?;
    Some(layout_with(r, |s, k| pilot(r, occ, s, k)))
}

/// The OFDM layout a receiver uses while it does not know the occupancy, over the union carrier range of the mode. Its known
/// cells are the pilots that have the same value in every occupancy that has the carrier. This leaves out the gain pilots
/// that table 59 boosts in some occupancies only.
pub fn search_layout(r: Robustness) -> FrameLayout {
    layout_with(r, |s, k| {
        let mut vals = (0..6u8)
            .filter(|&o| r.range(o).is_some_and(|(lo, hi)| k >= lo && k <= hi))
            .map(|o| pilot(r, o, s, k));
        let first = vals.next().flatten()?;
        vals.all(|v| v == Some(first)).then_some(first)
    })
}

fn layout_with(r: Robustness, known_cell: impl Fn(usize, i32) -> Option<Complex32>) -> FrameLayout {
    let (lo, hi) = r.union_range();
    let k = (hi - lo + 1) as usize;
    let mut known = vec![Vec::new(); r.nsym()];
    for (s, row) in known.iter_mut().enumerate() {
        for c in 0..k {
            if let Some(v) = known_cell(s, lo + c as i32) {
                row.push((c, v));
            }
        }
    }
    let null = (0..k).filter(|&c| r.unused(lo + c as i32)).collect();
    let max_shift = if r == Robustness::E {
        28
    } else {
        (12_500.0 / r.spacing()).ceil() as i32
    };
    FrameLayout {
        rate: r.rate(),
        nfft: r.nfft(),
        cp: r.cp(),
        cp0: r.cp(),
        nsym: r.nsym(),
        kmin: lo,
        kmax: hi,
        known,
        null,
        max_shift,
        adv: r.cp() / 2,
        sync_threshold: 0.45,
        guard_threshold: 0.35,
        search_taps: None,
        pair_band: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Control cells (clause 8.5) and cell counts (clauses 7.2.1 and 7.7).

/// The FAC cells of mode A: symbol and carriers (ES 201 980 clause 8.5.2.1, table 62).
pub const FAC_CELLS_A: [(usize, &[i32]); 12] = [
    (2, &[26, 46, 66, 86]),
    (3, &[10, 30, 50, 70, 90]),
    (4, &[14, 22, 34, 62, 74, 94]),
    (5, &[26, 38, 58, 66, 78]),
    (6, &[22, 30, 42, 62, 70, 82]),
    (7, &[26, 34, 46, 66, 74, 86]),
    (8, &[10, 30, 38, 50, 58, 70, 78, 90]),
    (9, &[14, 22, 34, 42, 62, 74, 82, 94]),
    (10, &[26, 38, 46, 66, 86]),
    (11, &[10, 30, 50, 70, 90]),
    (12, &[14, 34, 74, 94]),
    (13, &[38, 58, 78]),
];

/// The FAC cells of mode B: symbol and carriers (ES 201 980 clause 8.5.2.1, table 63).
pub const FAC_CELLS_B: [(usize, &[i32]); 12] = [
    (2, &[13, 25, 43, 55, 67]),
    (3, &[15, 27, 45, 57, 69]),
    (4, &[17, 29, 47, 59, 71]),
    (5, &[19, 31, 49, 61, 73]),
    (6, &[9, 21, 33, 51, 63, 75]),
    (7, &[11, 23, 35, 53, 65, 77]),
    (8, &[13, 25, 37, 55, 67, 79]),
    (9, &[15, 27, 39, 57, 69, 81]),
    (10, &[17, 29, 41, 59, 71, 83]),
    (11, &[19, 31, 43, 61, 73]),
    (12, &[21, 33, 45, 63, 75]),
    (13, &[23, 35, 47, 65, 77]),
];

/// The FAC cells of mode C: symbol and carriers (ES 201 980 clause 8.5.2.1, table 64).
pub const FAC_CELLS_C: [(usize, &[i32]); 16] = [
    (3, &[9, 21, 45, 57]),
    (4, &[23, 35, 47]),
    (5, &[13, 25, 37, 49]),
    (6, &[15, 27, 39, 51]),
    (7, &[5, 17, 29, 41, 53]),
    (8, &[7, 19, 31, 43, 55]),
    (9, &[9, 21, 45, 57]),
    (10, &[23, 35, 47]),
    (11, &[13, 25, 37, 49]),
    (12, &[15, 27, 39, 51]),
    (13, &[5, 17, 29, 41, 53]),
    (14, &[7, 19, 31, 43, 55]),
    (15, &[9, 21, 45, 57]),
    (16, &[23, 35, 47]),
    (17, &[13, 25, 37, 49]),
    (18, &[15, 27, 39, 51]),
];

/// The FAC cells of mode D: symbol and carriers (ES 201 980 clause 8.5.2.1, table 65).
pub const FAC_CELLS_D: [(usize, &[i32]); 20] = [
    (3, &[9, 18, 27]),
    (4, &[10, 19]),
    (5, &[11, 20, 29]),
    (6, &[12, 30]),
    (7, &[13, 22, 31]),
    (8, &[5, 14, 23, 32]),
    (9, &[6, 15, 24, 33]),
    (10, &[16, 25, 34]),
    (11, &[8, 17, 26, 35]),
    (12, &[9, 18, 27, 36]),
    (13, &[10, 19, 37]),
    (14, &[11, 20, 29]),
    (15, &[12, 30]),
    (16, &[13, 22, 31]),
    (17, &[5, 14, 23, 32]),
    (18, &[6, 15, 24, 33]),
    (19, &[16, 25, 34]),
    (20, &[8, 17, 26, 35]),
    (21, &[9, 18, 27, 36]),
    (22, &[10, 19, 37]),
];

const FAC_E1: &[i32] = &[-78, -62, -46, -30, -14, 2, 18, 34, 50, 66, 82];
const FAC_E2: &[i32] = &[-90, -74, -58, -42, -26, -10, 6, 22, 38, 54, 70, 86];
const FAC_E3: &[i32] = &[-86, -70, -54, -38, -22, -6, 10, 26, 42, 58, 74, 90];
const FAC_E4: &[i32] = &[-82, -66, -50, -34, -18, -2, 14, 30, 46, 62, 78];

/// The FAC cells of mode E: symbol and carriers (ES 201 980 clause 8.5.2.1, table 66). Symbols 5 to 25 repeat four rows of
/// the table in turn.
pub const FAC_CELLS_E: [(usize, &[i32]); 22] = [
    (5, FAC_E1),
    (6, FAC_E2),
    (7, FAC_E3),
    (8, FAC_E4),
    (9, FAC_E1),
    (10, FAC_E2),
    (11, FAC_E3),
    (12, FAC_E4),
    (13, FAC_E1),
    (14, FAC_E2),
    (15, FAC_E3),
    (16, FAC_E4),
    (17, FAC_E1),
    (18, FAC_E2),
    (19, FAC_E3),
    (20, FAC_E4),
    (21, FAC_E1),
    (22, FAC_E2),
    (23, FAC_E3),
    (24, FAC_E4),
    (25, FAC_E1),
    (26, &[-90, -74, -58]),
];

/// The FAC cells of a mode (tables 62 to 66), in the order the FAC fills them (clause 8.5.2.2).
pub fn fac_cells(r: Robustness) -> &'static [(usize, &'static [i32])] {
    match r {
        Robustness::A => &FAC_CELLS_A,
        Robustness::B => &FAC_CELLS_B,
        Robustness::C => &FAC_CELLS_C,
        Robustness::D => &FAC_CELLS_D,
        Robustness::E => &FAC_CELLS_E,
    }
}

/// The number of SDC symbols of each mode (clause 8.5.3.1): A and B 2, C and D 3, E 5.
pub const SDC_SYMBOLS: [usize; 5] = [2, 2, 3, 3, 5];

/// `N_SDC`, the SDC cells per super frame of each mode and occupancy (ES 201 980 clause 7.2.1.3, table 25).
pub const N_SDC: [[Option<u32>; 6]; 5] = [
    [
        Some(167),
        Some(190),
        Some(359),
        Some(405),
        Some(754),
        Some(846),
    ],
    [
        Some(130),
        Some(150),
        Some(282),
        Some(322),
        Some(588),
        Some(662),
    ],
    [None, None, None, Some(288), None, Some(607)],
    [None, None, None, Some(152), None, Some(332)],
    [Some(936), None, None, None, None, None],
];

/// The MSC cell counts of one mode and occupancy (clause 7.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MscCells {
    /// `N_SFA`, available MSC cells per super frame.
    pub available: u32,
    /// `N_SFU`, useful MSC cells per super frame.
    pub useful: u32,
    /// `N_MUX`, MSC cells per multiplex frame.
    pub mux: u32,
    /// `N_L`, the cells lost per super frame (dummy cells).
    pub loss: u32,
}

const fn mc(available: u32, useful: u32, mux: u32, loss: u32) -> Option<MscCells> {
    Some(MscCells {
        available,
        useful,
        mux,
        loss,
    })
}

/// The MSC cell counts of each mode and occupancy (ES 201 980 clause 7.7, tables 41 to 45).
pub const MSC_CELLS: [[Option<MscCells>; 6]; 5] = [
    [
        mc(3778, 3777, 1259, 1),
        mc(4268, 4266, 1422, 2),
        mc(7897, 7896, 2632, 1),
        mc(8877, 8877, 2959, 0),
        mc(16394, 16392, 5464, 2),
        mc(18354, 18354, 6118, 0),
    ],
    [
        mc(2900, 2898, 966, 2),
        mc(3330, 3330, 1110, 0),
        mc(6153, 6153, 2051, 0),
        mc(7013, 7011, 2337, 2),
        mc(12747, 12747, 4249, 0),
        mc(14323, 14322, 4774, 1),
    ],
    [
        None,
        None,
        None,
        mc(5532, 5532, 1844, 0),
        None,
        mc(11603, 11601, 3867, 2),
    ],
    [
        None,
        None,
        None,
        mc(3679, 3678, 1226, 1),
        None,
        mc(7819, 7818, 2606, 1),
    ],
    [mc(29842, 29840, 7460, 2), None, None, None, None, None],
];

/// Where the FAC, SDC and MSC cells of the frames of a super frame are: indices `s * ncarriers + (k - kmin)` into the cells
/// of the union layout of the mode, in the order the channels fill them (clauses 8.5.2.2, 8.5.3.2 and 8.6.2).
#[derive(Clone, Debug)]
pub struct CellMap {
    /// FAC cells (the same in every frame).
    pub fac: Vec<usize>,
    /// SDC cells (first frame of a super frame).
    pub sdc: Vec<usize>,
    /// MSC cells of each frame of the super frame.
    pub msc: Vec<Vec<usize>>,
}

impl CellMap {
    /// The map of mode `r` and occupancy `occ`, `None` when the mode does not allow the occupancy. A data cell is a cell that
    /// is not a pilot (AFS pilots included) and not an unused carrier. The SDC takes the data cells of the SDC symbols of the
    /// first frame. The FAC takes the cells of tables 62 to 66. The MSC takes the rest.
    pub fn new(r: Robustness, occ: u8) -> Option<CellMap> {
        let (olo, ohi) = r.range(occ)?;
        let (lo, hi) = r.union_range();
        let k = (hi - lo + 1) as usize;
        let at = |s: usize, kk: i32| s * k + (kk - lo) as usize;
        let mut fac = Vec::with_capacity(r.fac_cells());
        for &(s, ks) in fac_cells(r) {
            fac.extend(ks.iter().map(|&kk| at(s, kk)));
        }
        let mut sdc = Vec::new();
        let mut msc = vec![Vec::new(); r.frames_per_superframe()];
        for (f, cells) in msc.iter_mut().enumerate() {
            for s in 0..r.nsym() {
                for kk in olo..=ohi {
                    if r.unused(kk) || frame_pilot(r, occ, f, s, kk).is_some() {
                        continue;
                    }
                    let i = at(s, kk);
                    if f == 0 && s < r.sdc_symbols() {
                        sdc.push(i);
                    } else if !fac.contains(&i) {
                        cells.push(i);
                    }
                }
            }
        }
        Some(CellMap { fac, sdc, msc })
    }
    /// MSC cells per super frame (`N_SFA`).
    pub fn msc_per_superframe(&self) -> usize {
        self.msc.iter().map(Vec::len).sum()
    }
    /// MSC cells per multiplex frame (`N_MUX`): the super frame count divided by the frames, rounded down (clause 7.7).
    pub fn mux_cells(&self) -> usize {
        self.msc_per_superframe() / self.msc.len().max(1)
    }
}

/// The MSC constellation of an MSC mode code (mode E: 00 16-QAM, 11 4-QAM).
pub fn msc_qam(r: Robustness, msc_mode: u8) -> Qam {
    match (r == Robustness::E, msc_mode) {
        (false, 3) => Qam::Q16,
        (false, _) => Qam::Q64,
        (true, 3) => Qam::Q4,
        (true, _) => Qam::Q16,
    }
}

/// The SDC constellation: SDC mode 0 16-QAM, SDC mode 1 4-QAM, mode E always 4-QAM.
pub fn sdc_qam(r: Robustness, sdc_mode: u8) -> Qam {
    if r == Robustness::E || sdc_mode == 1 {
        Qam::Q4
    } else {
        Qam::Q16
    }
}

const fn r(rx: u8, ry: u8) -> Rate {
    Rate::new(rx, ry)
}

/// The MSC code rates of each level for a constellation and protection level (tables 29 to 32), with `RYlcm`. `None` for a
/// combination the standard does not have.
pub fn level_rates(mode: Robustness, q: Qam, protection: u8) -> Option<(&'static [Rate], usize)> {
    const Q4: [&[Rate]; 4] = [&[r(1, 4)], &[r(1, 3)], &[r(2, 5)], &[r(1, 2)]];
    const Q16: [&[Rate]; 2] = [&[r(1, 3), r(2, 3)], &[r(1, 2), r(3, 4)]];
    const Q16_E: [&[Rate]; 4] = [
        &[r(1, 6), r(1, 2)],
        &[r(1, 4), r(4, 7)],
        &[r(1, 3), r(2, 3)],
        &[r(1, 2), r(3, 4)],
    ];
    const Q64: [&[Rate]; 4] = [
        &[r(1, 4), r(1, 2), r(3, 4)],
        &[r(1, 3), r(2, 3), r(4, 5)],
        &[r(1, 2), r(3, 4), r(7, 8)],
        &[r(2, 3), r(4, 5), r(8, 9)],
    ];
    let p = protection as usize;
    let e = mode == Robustness::E;
    Some(match (q, e) {
        (Qam::Q4, true) => (*Q4.get(p)?, [4, 3, 5, 2][p]),
        (Qam::Q16, false) => (*Q16.get(p)?, [3, 4][p]),
        (Qam::Q16, true) => (*Q16_E.get(p)?, [6, 28, 3, 4][p]),
        (Qam::Q64, false) => (*Q64.get(p)?, [4, 15, 8, 45][p]),
        _ => return None,
    })
}

/// The SDC code rates (tables 36 to 38): 16-QAM 1/3 and 2/3, 4-QAM 1/2, and in mode E 1/2 (SDC mode 0) or 1/4 (SDC mode 1).
pub fn sdc_rates(mode: Robustness, sdc_mode: u8) -> &'static [Rate] {
    const R16: [Rate; 2] = [r(1, 3), r(2, 3)];
    const R4: [Rate; 1] = [r(1, 2)];
    const R4_QUARTER: [Rate; 1] = [r(1, 4)];
    match (mode == Robustness::E, sdc_mode) {
        (false, 0) => &R16,
        (true, 1) => &R4_QUARTER,
        _ => &R4,
    }
}

/// The FAC code rate (tables 39 and 40): 3/5, mode E 1/4.
pub fn fac_rate(mode: Robustness) -> Rate {
    if mode == Robustness::E {
        r(1, 4)
    } else {
        r(3, 5)
    }
}

/// Overall code rate in percent.
pub fn overall_rate_pct(rates: &[Rate]) -> f64 {
    100.0 * rates.iter().map(|x| x.value()).sum::<f64>() / rates.len().max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numerology() {
        for r in Robustness::ALL {
            assert!((r.frame_s() - if r == Robustness::E { 0.1 } else { 0.4 }).abs() < 1e-9);
        }
        let e = Robustness::E;
        assert!((e.spacing() - 444.444).abs() < 0.001);
        assert!(((e.nfft() + e.cp()) as f64 / e.rate() - 0.0025).abs() < 1e-12);
        assert_eq!(e.frames_per_superframe(), 4);
        assert_eq!(e.range(0), Some((-106, 106)));
        assert_eq!(e.fac_cells(), 244);
        assert_eq!(Robustness::B.fac_cells(), 65);
    }

    #[test]
    fn the_cells_of_a_frame_do_not_overlap() {
        for r in Robustness::ALL {
            for occ in 0..6 {
                if let Some(m) = CellMap::new(r, occ) {
                    for f in 0..r.frames_per_superframe() {
                        let mut all = m.fac.clone();
                        all.extend(&m.msc[f]);
                        if f == 0 {
                            all.extend(&m.sdc);
                        }
                        let n = all.len();
                        all.sort();
                        all.dedup();
                        assert_eq!(all.len(), n, "{r:?} {occ} {f}");
                    }
                }
            }
        }
    }
}
