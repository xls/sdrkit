//! The printed tables of both coders. The large ones are in [`full`] and [`half`]; the short ones are here.

pub mod full;
pub mod half;

/// Parity rows of the [23,12] Golay generator g_G (TIA-102.BABA section 7.3); row 0 belongs to the most significant
/// information bit, and the first parity bit is the most significant of the 11.
pub static GOLAY_PARITY: [u32; 12] = [
    0b110_0011_1010,
    0b011_0001_1101,
    0b111_1011_0100,
    0b011_1101_1010,
    0b001_1110_1101,
    0b110_1100_1100,
    0b011_0110_0110,
    0b001_1011_0011,
    0b110_1110_0011,
    0b101_0100_1011,
    0b100_1001_1111,
    0b100_0111_0101,
];

/// The extra parity bit of each row of the [24,12] Golay generator (TIA-102.BABA-1 clause 5.2).
pub static GOLAY24_EXTRA: [u32; 12] = [1, 1, 0, 0, 0, 1, 1, 1, 0, 1, 0, 1];

/// Parity rows of the [15,11] Hamming generator g_H (TIA-102.BABA section 7.3), in the same order as [`GOLAY_PARITY`].
pub static HAMMING_PARITY: [u32; 11] = [
    0b1111, 0b1110, 0b1101, 0b1100, 0b1011, 0b1010, 0b1001, 0b0111, 0b0110, 0b0101, 0b0011,
];

/// Step size of a higher-order DCT coefficient in units of sigma, for B = 1..10 bits, index B - 1 (TIA-102.BABA Table 3).
pub static HOC_STEP: [f64; 10] = [1.2, 0.85, 0.65, 0.40, 0.28, 0.15, 0.08, 0.04, 0.02, 0.01];

/// Sigma of the coefficient C_{i,k} for k = 2..10, index k - 2 (TIA-102.BABA Table 4).
pub static HOC_SIGMA: [f64; 9] = [0.307, 0.241, 0.207, 0.190, 0.179, 0.173, 0.165, 0.170, 0.170];

/// Single tones of the half-rate tone frame (TIA-102.BABA-1 Annex J): (first ID, last ID, f0 per unit of ID in Hz, harmonic
/// number). The tone is harmonic `l` of f0.
pub static TONE_SINGLE: [(u8, u8, f64, u8); 10] = [
    (5, 12, 31.250, 1),
    (13, 25, 15.625, 2),
    (26, 38, 10.417, 3),
    (39, 51, 7.8125, 4),
    (52, 64, 6.2500, 5),
    (65, 76, 5.2803, 6),
    (77, 89, 4.4643, 7),
    (90, 102, 3.9063, 8),
    (103, 115, 3.4722, 9),
    (116, 122, 3.1250, 10),
];

/// Dual tones of the half-rate tone frame for ID = 128..163, index ID - 128 (TIA-102.BABA-1 Annex J): (f0 in Hz, l1, l2).
pub static TONE_PAIR: [(f64, u8, u8); 36] = [
    (78.5, 12, 17),
    (173.48, 4, 7),
    (70.0, 10, 19),
    (87.0, 8, 17),
    (109.95, 7, 11),
    (191.68, 4, 7),
    (70.17, 11, 21),
    (71.06, 12, 17),
    (121.58, 7, 11),
    (212.0, 4, 7),
    (116.41, 6, 14),
    (96.15, 8, 17),
    (71.0, 12, 23),
    (234.26, 4, 7),
    (134.38, 7, 9),
    (134.35, 7, 11),
    (68.33, 12, 17),
    (150.89, 4, 7),
    (67.82, 9, 17),
    (86.5, 7, 15),
    (95.79, 7, 11),
    (166.92, 4, 7),
    (67.7, 10, 19),
    (74.74, 10, 14),
    (105.90, 7, 11),
    (92.78, 8, 14),
    (101.55, 6, 14),
    (84.02, 8, 17),
    (67.83, 11, 21),
    (102.3, 8, 14),
    (117.0, 7, 9),
    (117.49, 7, 11),
    (87.78, 4, 5),
    (70.83, 6, 7),
    (122.0, 4, 5),
    (70.0, 5, 7),
];

/// The zero-amplitude tone, ID 255 (TIA-102.BABA-1 Annex J): f0 in Hz; it has no harmonic.
pub const TONE_ZERO_F0: f64 = 250.0;
