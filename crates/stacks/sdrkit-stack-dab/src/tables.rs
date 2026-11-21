//! The fixed tables of DAB transmission mode I: geometry, the PRS (the synchronisation symbol after the null), the frequency
//! interleaver and the protection profiles (EEP and the 64 UEP rows).
//!
//! Standard: EN 300 401 clauses 11, 14.3 and 14.6.

/// Useful symbol length Tu at 2.048 Msps (1 ms; the carrier spacing is 1 kHz).
pub const TU: usize = 2048;
/// Guard interval Tg in samples.
pub const TG: usize = 504;
/// Symbol length Ts = Tu + Tg.
pub const TS: usize = TU + TG;
/// Null symbol length.
pub const TNULL: usize = 2656;
/// OFDM symbols after the null: the PRS, 3 FIC and 72 MSC symbols.
pub const SYMBOLS: usize = 76;
/// Samples per transmission frame (96 ms).
pub const FRAME: usize = TNULL + SYMBOLS * TS;
/// Active carriers.
pub const CARRIERS: usize = 1536;
/// Half the active carriers: carriers run from -768 to +768 without 0.
pub const HALF: i32 = 768;
/// Bits per OFDM symbol.
pub const SYMBOL_BITS: usize = 2 * CARRIERS;
/// FIC bits per frame (3 symbols).
pub const FIC_BITS: usize = 3 * SYMBOL_BITS;
/// Coded FIC bits per CIF (one FIB group of 3 FIBs).
pub const FIC_BLOCK_BITS: usize = 2304;
/// Bits per CIF.
pub const CIF_BITS: usize = 55_296;
/// Capacity units per CIF.
pub const CU_PER_CIF: usize = 864;
/// Bits per capacity unit.
pub const CU_BITS: usize = 64;
/// The sample rate of mode I.
pub const RATE: f64 = 2_048_000.0;
/// The CIF counter's modulus.
pub const CIF_MOD: u32 = 5000;

/// The relation between carrier index ranges and (k', i, n) for the PRS (EN 300 401 table 23).
const PRS_TABLE: [(i32, usize, u8); 48] = [
    (-768, 0, 1),
    (-736, 1, 2),
    (-704, 2, 0),
    (-672, 3, 1),
    (-640, 0, 3),
    (-608, 1, 2),
    (-576, 2, 2),
    (-544, 3, 3),
    (-512, 0, 2),
    (-480, 1, 1),
    (-448, 2, 2),
    (-416, 3, 3),
    (-384, 0, 1),
    (-352, 1, 2),
    (-320, 2, 3),
    (-288, 3, 3),
    (-256, 0, 2),
    (-224, 1, 2),
    (-192, 2, 2),
    (-160, 3, 1),
    (-128, 0, 1),
    (-96, 1, 3),
    (-64, 2, 1),
    (-32, 3, 2),
    (1, 0, 3),
    (33, 3, 1),
    (65, 2, 1),
    (97, 1, 1),
    (129, 0, 2),
    (161, 3, 2),
    (193, 2, 1),
    (225, 1, 0),
    (257, 0, 2),
    (289, 3, 2),
    (321, 2, 3),
    (353, 1, 3),
    (385, 0, 0),
    (417, 3, 2),
    (449, 2, 1),
    (481, 1, 3),
    (513, 0, 3),
    (545, 3, 3),
    (577, 2, 3),
    (609, 1, 0),
    (641, 0, 3),
    (673, 3, 0),
    (705, 2, 1),
    (737, 1, 1),
];

/// The time-frequency-phase parameter h (EN 300 401 table 24); the second 16 columns repeat the first.
const PRS_H: [[u8; 16]; 4] = [
    [0, 2, 0, 0, 0, 0, 1, 1, 2, 0, 0, 0, 2, 2, 1, 1],
    [0, 3, 2, 3, 0, 1, 3, 0, 2, 1, 2, 3, 2, 3, 3, 0],
    [0, 0, 0, 2, 0, 2, 1, 3, 2, 2, 0, 2, 2, 0, 1, 3],
    [0, 1, 2, 1, 0, 3, 3, 2, 2, 3, 2, 1, 2, 1, 3, 2],
];

/// The phase of carrier `k` (−768 … 768, not 0) of the PRS, in quarter turns (0 … 3).
pub fn prs_quarter_turns(k: i32) -> u8 {
    debug_assert!(k != 0 && (-HALF..=HALF).contains(&k));
    let row = PRS_TABLE
        .iter()
        .rposition(|r| r.0 <= k)
        .expect("carrier in range");
    let (kp, i, n) = PRS_TABLE[row];
    (PRS_H[i][((k - kp) as usize) % 16] + n) % 4
}

/// The PRS as unit-magnitude carriers, indexed by FFT bin (k mod 2048); bins without a carrier are 0.
pub fn prs_bins() -> Vec<num_complex::Complex<f32>> {
    let mut out = vec![num_complex::Complex::new(0.0, 0.0); TU];
    for k in (-HALF..=HALF).filter(|&k| k != 0) {
        out[bin(k)] = quarter(prs_quarter_turns(k));
    }
    out
}

/// The unit phasor of `q` quarter turns.
pub fn quarter(q: u8) -> num_complex::Complex<f32> {
    match q % 4 {
        0 => num_complex::Complex::new(1.0, 0.0),
        1 => num_complex::Complex::new(0.0, 1.0),
        2 => num_complex::Complex::new(-1.0, 0.0),
        _ => num_complex::Complex::new(0.0, -1.0),
    }
}

/// The FFT bin of carrier `k`.
pub fn bin(k: i32) -> usize {
    k.rem_euclid(TU as i32) as usize
}

/// The frequency interleaver of mode I: entry n is the carrier that carries the n-th bit pair (EN 300 401 clause 14.6.1).
pub fn frequency_interleaver() -> Vec<i32> {
    let mut k = Vec::with_capacity(CARRIERS);
    sdrkit_util_bits::interleave::dab_frequency_mode1(&mut k);
    k.into_iter().map(i32::from).collect()
}

/// One row of the UEP table (EN 300 401 table 8 and table 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Uep {
    /// Sub-channel size in CU.
    pub size_cu: u16,
    /// Protection level 1 … 5.
    pub level: u8,
    /// Bit rate in kbit/s.
    pub bitrate: u16,
    /// The four runs of 128-position blocks.
    pub l: [u16; 4],
    /// Their puncturing indices (0 where the run is empty).
    pub pi: [u8; 4],
}

/// The 64 UEP profiles, by table index (EN 300 401 tables 8 and 15).
pub static UEP: [Uep; 64] = [
    Uep {
        size_cu: 16,
        level: 5,
        bitrate: 32,
        l: [3, 4, 17, 0],
        pi: [5, 3, 2, 0],
    }, // 0
    Uep {
        size_cu: 21,
        level: 4,
        bitrate: 32,
        l: [3, 3, 18, 0],
        pi: [11, 6, 5, 0],
    }, // 1
    Uep {
        size_cu: 24,
        level: 3,
        bitrate: 32,
        l: [3, 4, 14, 3],
        pi: [15, 9, 6, 8],
    }, // 2
    Uep {
        size_cu: 29,
        level: 2,
        bitrate: 32,
        l: [3, 4, 14, 3],
        pi: [22, 13, 8, 13],
    }, // 3
    Uep {
        size_cu: 35,
        level: 1,
        bitrate: 32,
        l: [3, 5, 13, 3],
        pi: [24, 17, 12, 17],
    }, // 4
    Uep {
        size_cu: 24,
        level: 5,
        bitrate: 48,
        l: [4, 3, 26, 3],
        pi: [5, 4, 2, 3],
    }, // 5
    Uep {
        size_cu: 29,
        level: 4,
        bitrate: 48,
        l: [3, 4, 26, 3],
        pi: [9, 6, 4, 6],
    }, // 6
    Uep {
        size_cu: 35,
        level: 3,
        bitrate: 48,
        l: [3, 4, 26, 3],
        pi: [15, 10, 6, 9],
    }, // 7
    Uep {
        size_cu: 42,
        level: 2,
        bitrate: 48,
        l: [3, 4, 26, 3],
        pi: [24, 14, 8, 15],
    }, // 8
    Uep {
        size_cu: 52,
        level: 1,
        bitrate: 48,
        l: [3, 5, 25, 3],
        pi: [24, 18, 13, 18],
    }, // 9
    Uep {
        size_cu: 29,
        level: 5,
        bitrate: 56,
        l: [6, 10, 23, 3],
        pi: [5, 4, 2, 3],
    }, // 10
    Uep {
        size_cu: 35,
        level: 4,
        bitrate: 56,
        l: [6, 10, 23, 3],
        pi: [9, 6, 4, 5],
    }, // 11
    Uep {
        size_cu: 42,
        level: 3,
        bitrate: 56,
        l: [6, 12, 21, 3],
        pi: [16, 7, 6, 9],
    }, // 12
    Uep {
        size_cu: 52,
        level: 2,
        bitrate: 56,
        l: [6, 10, 23, 3],
        pi: [23, 13, 8, 13],
    }, // 13
    Uep {
        size_cu: 32,
        level: 5,
        bitrate: 64,
        l: [6, 9, 31, 2],
        pi: [5, 3, 2, 3],
    }, // 14
    Uep {
        size_cu: 42,
        level: 4,
        bitrate: 64,
        l: [6, 9, 33, 0],
        pi: [11, 6, 5, 0],
    }, // 15
    Uep {
        size_cu: 48,
        level: 3,
        bitrate: 64,
        l: [6, 12, 27, 3],
        pi: [16, 8, 6, 9],
    }, // 16
    Uep {
        size_cu: 58,
        level: 2,
        bitrate: 64,
        l: [6, 10, 29, 3],
        pi: [23, 13, 8, 13],
    }, // 17
    Uep {
        size_cu: 70,
        level: 1,
        bitrate: 64,
        l: [6, 11, 28, 3],
        pi: [24, 18, 12, 18],
    }, // 18
    Uep {
        size_cu: 40,
        level: 5,
        bitrate: 80,
        l: [6, 10, 41, 3],
        pi: [6, 3, 2, 3],
    }, // 19
    Uep {
        size_cu: 52,
        level: 4,
        bitrate: 80,
        l: [6, 10, 41, 3],
        pi: [11, 6, 5, 6],
    }, // 20
    Uep {
        size_cu: 58,
        level: 3,
        bitrate: 80,
        l: [6, 11, 40, 3],
        pi: [16, 8, 6, 7],
    }, // 21
    Uep {
        size_cu: 70,
        level: 2,
        bitrate: 80,
        l: [6, 10, 41, 3],
        pi: [23, 13, 8, 13],
    }, // 22
    Uep {
        size_cu: 84,
        level: 1,
        bitrate: 80,
        l: [6, 10, 41, 3],
        pi: [24, 17, 12, 18],
    }, // 23
    Uep {
        size_cu: 48,
        level: 5,
        bitrate: 96,
        l: [7, 9, 53, 3],
        pi: [5, 4, 2, 4],
    }, // 24
    Uep {
        size_cu: 58,
        level: 4,
        bitrate: 96,
        l: [7, 10, 52, 3],
        pi: [9, 6, 4, 6],
    }, // 25
    Uep {
        size_cu: 70,
        level: 3,
        bitrate: 96,
        l: [6, 12, 51, 3],
        pi: [16, 9, 6, 10],
    }, // 26
    Uep {
        size_cu: 84,
        level: 2,
        bitrate: 96,
        l: [6, 10, 53, 3],
        pi: [22, 12, 9, 12],
    }, // 27
    Uep {
        size_cu: 104,
        level: 1,
        bitrate: 96,
        l: [6, 13, 50, 3],
        pi: [24, 18, 13, 19],
    }, // 28
    Uep {
        size_cu: 58,
        level: 5,
        bitrate: 112,
        l: [14, 17, 50, 3],
        pi: [5, 4, 2, 5],
    }, // 29
    Uep {
        size_cu: 70,
        level: 4,
        bitrate: 112,
        l: [11, 21, 49, 3],
        pi: [9, 6, 4, 8],
    }, // 30
    Uep {
        size_cu: 84,
        level: 3,
        bitrate: 112,
        l: [11, 23, 47, 3],
        pi: [16, 8, 6, 9],
    }, // 31
    Uep {
        size_cu: 104,
        level: 2,
        bitrate: 112,
        l: [11, 21, 49, 3],
        pi: [23, 12, 9, 14],
    }, // 32
    Uep {
        size_cu: 64,
        level: 5,
        bitrate: 128,
        l: [12, 19, 62, 3],
        pi: [5, 3, 2, 4],
    }, // 33
    Uep {
        size_cu: 84,
        level: 4,
        bitrate: 128,
        l: [11, 21, 61, 3],
        pi: [11, 6, 5, 7],
    }, // 34
    Uep {
        size_cu: 96,
        level: 3,
        bitrate: 128,
        l: [11, 22, 60, 3],
        pi: [16, 9, 6, 10],
    }, // 35
    Uep {
        size_cu: 116,
        level: 2,
        bitrate: 128,
        l: [11, 21, 61, 3],
        pi: [22, 12, 9, 14],
    }, // 36
    Uep {
        size_cu: 140,
        level: 1,
        bitrate: 128,
        l: [11, 20, 62, 3],
        pi: [24, 17, 13, 19],
    }, // 37
    Uep {
        size_cu: 80,
        level: 5,
        bitrate: 160,
        l: [11, 19, 87, 3],
        pi: [5, 4, 2, 4],
    }, // 38
    Uep {
        size_cu: 104,
        level: 4,
        bitrate: 160,
        l: [11, 23, 83, 3],
        pi: [11, 6, 5, 9],
    }, // 39
    Uep {
        size_cu: 116,
        level: 3,
        bitrate: 160,
        l: [11, 24, 82, 3],
        pi: [16, 8, 6, 11],
    }, // 40
    Uep {
        size_cu: 140,
        level: 2,
        bitrate: 160,
        l: [11, 21, 85, 3],
        pi: [22, 11, 9, 13],
    }, // 41
    Uep {
        size_cu: 168,
        level: 1,
        bitrate: 160,
        l: [11, 22, 84, 3],
        pi: [24, 18, 12, 19],
    }, // 42
    Uep {
        size_cu: 96,
        level: 5,
        bitrate: 192,
        l: [11, 20, 110, 3],
        pi: [6, 4, 2, 5],
    }, // 43
    Uep {
        size_cu: 116,
        level: 4,
        bitrate: 192,
        l: [11, 22, 108, 3],
        pi: [10, 6, 4, 9],
    }, // 44
    Uep {
        size_cu: 140,
        level: 3,
        bitrate: 192,
        l: [11, 24, 106, 3],
        pi: [16, 10, 6, 11],
    }, // 45
    Uep {
        size_cu: 168,
        level: 2,
        bitrate: 192,
        l: [11, 20, 110, 3],
        pi: [22, 13, 9, 13],
    }, // 46
    Uep {
        size_cu: 208,
        level: 1,
        bitrate: 192,
        l: [11, 21, 109, 3],
        pi: [24, 20, 13, 24],
    }, // 47
    Uep {
        size_cu: 116,
        level: 5,
        bitrate: 224,
        l: [12, 22, 131, 3],
        pi: [8, 6, 2, 6],
    }, // 48
    Uep {
        size_cu: 140,
        level: 4,
        bitrate: 224,
        l: [12, 26, 127, 3],
        pi: [12, 8, 4, 11],
    }, // 49
    Uep {
        size_cu: 168,
        level: 3,
        bitrate: 224,
        l: [11, 20, 134, 3],
        pi: [16, 10, 7, 9],
    }, // 50
    Uep {
        size_cu: 208,
        level: 2,
        bitrate: 224,
        l: [11, 22, 132, 3],
        pi: [24, 16, 10, 15],
    }, // 51
    Uep {
        size_cu: 232,
        level: 1,
        bitrate: 224,
        l: [11, 24, 130, 3],
        pi: [24, 20, 12, 20],
    }, // 52
    Uep {
        size_cu: 128,
        level: 5,
        bitrate: 256,
        l: [11, 24, 154, 3],
        pi: [6, 5, 2, 5],
    }, // 53
    Uep {
        size_cu: 168,
        level: 4,
        bitrate: 256,
        l: [11, 24, 154, 3],
        pi: [12, 9, 5, 10],
    }, // 54
    Uep {
        size_cu: 192,
        level: 3,
        bitrate: 256,
        l: [11, 27, 151, 3],
        pi: [16, 10, 7, 10],
    }, // 55
    Uep {
        size_cu: 232,
        level: 2,
        bitrate: 256,
        l: [11, 22, 156, 3],
        pi: [24, 14, 10, 13],
    }, // 56
    Uep {
        size_cu: 280,
        level: 1,
        bitrate: 256,
        l: [11, 26, 152, 3],
        pi: [24, 19, 14, 18],
    }, // 57
    Uep {
        size_cu: 160,
        level: 5,
        bitrate: 320,
        l: [11, 26, 200, 3],
        pi: [8, 5, 2, 6],
    }, // 58
    Uep {
        size_cu: 208,
        level: 4,
        bitrate: 320,
        l: [11, 25, 201, 3],
        pi: [13, 9, 5, 10],
    }, // 59
    Uep {
        size_cu: 280,
        level: 2,
        bitrate: 320,
        l: [11, 26, 200, 3],
        pi: [24, 17, 9, 17],
    }, // 60
    Uep {
        size_cu: 192,
        level: 5,
        bitrate: 384,
        l: [11, 27, 247, 3],
        pi: [8, 6, 2, 7],
    }, // 61
    Uep {
        size_cu: 280,
        level: 3,
        bitrate: 384,
        l: [11, 24, 250, 3],
        pi: [16, 9, 7, 10],
    }, // 62
    Uep {
        size_cu: 416,
        level: 1,
        bitrate: 384,
        l: [12, 28, 245, 3],
        pi: [24, 20, 14, 23],
    }, // 63
];

/// A protection profile: runs of (blocks, PI) followed by the tail, then padding to the sub-channel size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    /// (number of 128-position blocks, puncturing index) in order.
    pub runs: Vec<(usize, usize)>,
    /// The information bits per logical frame (24 × bit rate).
    pub info_bits: usize,
    /// The sub-channel size in CU.
    pub size_cu: usize,
}

impl Profile {
    /// The coded bits before padding.
    pub fn coded_bits(&self) -> usize {
        self.runs
            .iter()
            .map(|&(l, pi)| l * 4 * (8 + pi))
            .sum::<usize>()
            + 12
    }
}

/// How a sub-channel is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protection {
    /// UEP with this table index.
    Uep(u8),
    /// EEP: option (0 = A, 1 = B) and level (1 … 4).
    Eep {
        /// 0 = A, 1 = B.
        option: u8,
        /// 1 … 4.
        level: u8,
    },
}

impl Protection {
    /// "EEP 3-A" or "UEP 12".
    pub fn text(&self) -> String {
        match *self {
            Protection::Uep(i) => format!("UEP {i}"),
            Protection::Eep { option, level } => {
                format!("EEP {level}-{}", if option == 0 { 'A' } else { 'B' })
            }
        }
    }
}

/// The EEP profile of `level` (1 … 4) and `option` (0 = A, 1 = B) at `bitrate` kbit/s, with its size; `None` for a combination the
/// standard does not allow (a rate that is not a multiple of 8 (A) or 32 (B)).
pub fn eep_profile(option: u8, level: u8, bitrate: usize) -> Option<Profile> {
    let (unit, n) = match option {
        0 => (8, bitrate / 8),
        1 => (32, bitrate / 32),
        _ => return None,
    };
    if n == 0 || !bitrate.is_multiple_of(unit) || !(1..=4).contains(&level) {
        return None;
    }
    let (size, runs) = match (option, level) {
        (0, 1) => (12 * n, vec![(6 * n - 3, 24), (3, 23)]),
        // At 8 kbit/s, 2-A has its own runs (EN 300 401 table 18, note).
        (0, 2) if n == 1 => (8, vec![(5, 13), (1, 12)]),
        (0, 2) => (8 * n, vec![(2 * n - 3, 14), (4 * n + 3, 13)]),
        (0, 3) => (6 * n, vec![(6 * n - 3, 8), (3, 7)]),
        (0, 4) => (4 * n, vec![(4 * n - 3, 3), (2 * n + 3, 2)]),
        (1, 1) => (27 * n, vec![(24 * n - 3, 10), (3, 9)]),
        (1, 2) => (21 * n, vec![(24 * n - 3, 6), (3, 5)]),
        (1, 3) => (18 * n, vec![(24 * n - 3, 4), (3, 3)]),
        (1, 4) => (15 * n, vec![(24 * n - 3, 2), (3, 1)]),
        _ => return None,
    };
    Some(Profile {
        runs,
        info_bits: 24 * bitrate,
        size_cu: size,
    })
}

/// The bit rate of an EEP sub-channel from its size in CU (the inverse of [`eep_profile`]); `None` when the size does not fit.
pub fn eep_bitrate(option: u8, level: u8, size_cu: usize) -> Option<usize> {
    let per = match (option, level) {
        (0, 1) => 12,
        (0, 2) => 8,
        (0, 3) => 6,
        (0, 4) => 4,
        (1, 1) => 27,
        (1, 2) => 21,
        (1, 3) => 18,
        (1, 4) => 15,
        _ => return None,
    };
    if size_cu == 0 || !size_cu.is_multiple_of(per) {
        return None;
    }
    let n = size_cu / per;
    Some(if option == 0 { 8 * n } else { 32 * n })
}

/// The UEP profile of table index `index`.
pub fn uep_profile(index: usize) -> Option<Profile> {
    let u = UEP.get(index)?;
    let runs = (0..4)
        .filter(|&i| u.l[i] > 0)
        .map(|i| (u.l[i] as usize, u.pi[i] as usize))
        .collect();
    Some(Profile {
        runs,
        info_bits: 24 * u.bitrate as usize,
        size_cu: u.size_cu as usize,
    })
}

/// The profile of a protection at a bit rate (EEP) or by its index (UEP).
pub fn profile(p: Protection, bitrate: usize) -> Option<Profile> {
    match p {
        Protection::Uep(i) => uep_profile(i as usize),
        Protection::Eep { option, level } => eep_profile(option, level, bitrate),
    }
}

/// The FIC profile: 21 blocks with PI 16, 3 with PI 15, then the tail (2304 bits per 768 information bits).
pub fn fic_profile() -> Profile {
    Profile {
        runs: vec![(21, 16), (3, 15)],
        info_bits: 768,
        size_cu: 36,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry() {
        assert_eq!(FRAME, 196_608);
        assert_eq!(FIC_BITS, 9216);
        assert_eq!(CIF_BITS, CU_PER_CIF * CU_BITS);
        assert_eq!(18 * SYMBOL_BITS, CIF_BITS);
    }

    #[test]
    fn interleaver_is_a_permutation_starting_as_printed() {
        let f = frequency_interleaver();
        assert_eq!(f.len(), 1536);
        assert_eq!(&f[..8], &[-513, -14, 329, 692, -733, 13, 680, 273]);
        let mut s = f.clone();
        s.sort();
        let want: Vec<i32> = (-768..=768).filter(|&k| k != 0).collect();
        assert_eq!(s, want);
    }

    #[test]
    fn phase_reference_carriers() {
        let z = prs_bins();
        assert_eq!(z[bin(-768)], quarter(1));
        assert_eq!(z[bin(-767)], quarter(3));
        assert_eq!(z[bin(1)], quarter(3));
        assert_eq!(z[0].norm(), 0.0);
        assert_eq!(z[1024].norm(), 0.0);
        assert_eq!(z.iter().filter(|c| c.norm() > 0.5).count(), 1536);
    }

    #[test]
    fn the_fic_profile_codes_768_bits_into_2304() {
        assert_eq!(fic_profile().coded_bits(), 2304);
    }

    #[test]
    fn eep_sizes() {
        assert_eq!(eep_profile(0, 3, 48).unwrap().size_cu, 36);
        assert_eq!(eep_profile(0, 2, 128).unwrap().size_cu, 128);
        assert_eq!(eep_profile(1, 3, 96).unwrap().size_cu, 54);
        for opt in 0..2u8 {
            for lvl in 1..=4u8 {
                for br in (8..=384).step_by(8) {
                    if let Some(p) = eep_profile(opt, lvl, br) {
                        assert_eq!(p.runs.iter().map(|r| r.0).sum::<usize>() * 32, p.info_bits);
                        assert_eq!(p.coded_bits(), p.size_cu * 64, "{opt} {lvl} {br}");
                        assert_eq!(eep_bitrate(opt, lvl, p.size_cu), Some(br));
                    }
                }
            }
        }
    }

    #[test]
    fn every_uep_row_fits() {
        for (i, u) in UEP.iter().enumerate() {
            let p = uep_profile(i).unwrap();
            assert_eq!(
                32 * p.runs.iter().map(|r| r.0).sum::<usize>(),
                p.info_bits,
                "row {i}"
            );
            let pad = u.size_cu as usize * 64 - p.coded_bits();
            assert!(pad < 64, "row {i}");
        }
    }
}
