//! The ISDB-T OFDM frame layout: the segment order in the band, the roles of the carriers of a segment (scattered pilots,
//! continual pilot, TMCC, AC1, AC2, data), the pilot sequence, the frequency interleaving, the time interleaver and the bit
//! interleaver. A [`Band`] of 13 segments is ISDB-T; a band of 1 or 3 segments is the centre of the 13 places (ISDB-TSB).
//!
//! Standard: ARIB STD-B31 clauses 3.9 to 3.13 (tables 3-13 to 3-16); ITU-R BS.1114 Annex 3 (System F).

use super::tmcc::Config;
use num_complex::Complex32;
use sdrkit_util_bits::lfsr::Lfsr;

/// Segment numbers at the frequency positions 0..12 (low to high). ARIB STD-B31 clause 3.14.
pub const SEG_AT_POS: [usize; 13] = [11, 9, 7, 5, 3, 1, 0, 2, 4, 6, 8, 10, 12];

/// The frequency position of segment number `s`.
pub fn pos_of_seg(s: usize) -> usize {
    SEG_AT_POS.iter().position(|&x| x == s).unwrap_or(0)
}

/// The value of the continual pilot at the upper edge of a narrow band (the first carrier of the next segment place).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EdgePilot {
    /// The pilot sequence's value at that carrier, as the ISDB-T segment at the next place has it.
    #[default]
    Sequence,
    /// +4/3.
    Plus,
    /// −4/3.
    Minus,
}

/// The segments a band holds: 13 (ISDB-T) or the centre 1 or 3 of the 13 places (ISDB-TSB), and its upper-edge pilot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    /// Segments: 1, 3 or 13.
    pub ns: usize,
    /// The upper-edge pilot rule (a band of 13 always uses the sequence).
    pub edge: EdgePilot,
}

impl Default for Band {
    fn default() -> Self {
        Band::FULL
    }
}

impl Band {
    /// The 13 segments of ISDB-T.
    pub const FULL: Band = Band {
        ns: 13,
        edge: EdgePilot::Sequence,
    };

    /// A band of `ns` segments (1, 3 or 13; other counts give 13).
    pub fn of(ns: usize, edge: EdgePilot) -> Band {
        Band {
            ns: if ns == 1 || ns == 3 { ns } else { 13 },
            edge: if ns == 1 || ns == 3 {
                edge
            } else {
                EdgePilot::Sequence
            },
        }
    }

    /// The frequency position of the band's lowest segment among the 13 places.
    pub fn first_pos(&self) -> usize {
        (13 - self.ns) / 2
    }

    /// The FFT size of a mode: 2048, 4096, 8192 for 13 segments; a fourth for 3; an eighth for 1 (BS.1114 Annex 3 table 9).
    pub fn fft_size(&self, mode: u8) -> usize {
        (1024 << mode) / self.div()
    }

    /// The divisor of the ISDB-T IFFT rate and FFT size: 1, 4 or 8.
    pub fn div(&self) -> usize {
        match self.ns {
            1 => 8,
            3 => 4,
            _ => 1,
        }
    }

    /// Carriers in the band: `ns` segments and the continual pilot above them.
    pub fn carriers(&self, mode: u8) -> usize {
        self.ns * seg_carriers(mode) + 1
    }

    /// The IFFT rate of a raster `width_mhz` wide.
    pub fn ifft_rate(&self, width_mhz: f64) -> f64 {
        super::ifft_rate(width_mhz) / self.div() as f64
    }

    /// The occupied width in Hz with the carrier spacing of mode 1 (the widest): `ns W / 14 + W / 1512`.
    pub fn occupied_hz(&self, width_mhz: f64) -> f64 {
        let bws = width_mhz * 1e6 / 14.0;
        self.ns as f64 * bws + bws / 108.0
    }

    /// The segment numbers in the band, low to high in frequency.
    pub fn segments(&self) -> &'static [usize] {
        &SEG_AT_POS[self.first_pos()..self.first_pos() + self.ns]
    }
}

/// Carriers per segment of a mode.
pub fn seg_carriers(mode: u8) -> usize {
    108 << (mode - 1)
}

/// The pilot amplitude.
pub const PILOT: f32 = 4.0 / 3.0;

/// The pilot sequence w_i (the register of ARIB STD-B31 figure 3-31, the same sequence as the DVB-T pilots), from all ones at
/// carrier 0 of segment 11.
pub const PILOTS: Lfsr = sdrkit_util_bits::seq::DVBT_PILOTS;

/// The pilot sequence across the band: `n` bits, one per carrier from the lowest. Table 3-16 of ARIB STD-B31 lists the same
/// register state at the start of each segment.
pub fn pilot_prbs(n: usize) -> Vec<u8> {
    let mut l = PILOTS;
    let mut v = vec![0u8; n];
    l.fill_bits(&mut v);
    v
}

/// The TMCC, AC1 and AC2 carrier positions (0-based inside the segment) of a segment.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SegTable {
    /// TMCC carriers.
    pub tmcc: Vec<u16>,
    /// AC1 carriers.
    pub ac1: Vec<u16>,
    /// AC2 carriers (differential segments only).
    pub ac2: Vec<u16>,
}

// ARIB STD-B31 tables 3-14 (differential segments) and 3-15 (coherent segments). Each row lists one carrier role for the
// segments in frequency order (segment numbers 11, 9, 7, 5, 3, 1, 0, 2, 4, 6, 8, 10, 12). The continual pilot of a differential
// segment is carrier 0.
const AC1_DIFF_1: [[u16; 13]; 2] = [
    [10, 53, 61, 11, 20, 74, 35, 76, 4, 40, 8, 7, 98],
    [28, 83, 100, 101, 40, 100, 79, 97, 89, 89, 64, 89, 101],
];
const AC2_DIFF_1: [[u16; 13]; 4] = [
    [3, 3, 29, 28, 23, 30, 3, 5, 13, 72, 36, 25, 10],
    [45, 15, 41, 45, 63, 81, 72, 18, 93, 95, 48, 30, 30],
    [59, 40, 84, 81, 85, 92, 85, 57, 98, 100, 52, 42, 55],
    [77, 58, 93, 91, 105, 103, 89, 92, 102, 105, 74, 104, 81],
];
const TMCC_DIFF_1: [[u16; 13]; 5] = [
    [13, 25, 4, 36, 10, 7, 49, 31, 16, 5, 78, 34, 23],
    [50, 63, 7, 48, 28, 25, 61, 39, 30, 10, 82, 48, 37],
    [70, 73, 17, 55, 44, 47, 96, 47, 37, 21, 85, 54, 51],
    [83, 80, 51, 59, 47, 60, 99, 65, 74, 44, 98, 70, 68],
    [87, 93, 71, 86, 54, 87, 104, 72, 83, 61, 102, 101, 105],
];
const AC1_DIFF_2: [[u16; 13]; 4] = [
    [10, 61, 20, 35, 4, 8, 98, 53, 11, 74, 76, 40, 7],
    [28, 100, 40, 79, 89, 64, 101, 83, 101, 100, 97, 89, 89],
    [
        161, 119, 182, 184, 148, 115, 118, 169, 128, 143, 112, 116, 206,
    ],
    [
        191, 209, 208, 205, 197, 197, 136, 208, 148, 187, 197, 172, 209,
    ],
];
const AC2_DIFF_2: [[u16; 13]; 9] = [
    [3, 29, 23, 3, 13, 36, 10, 3, 28, 30, 5, 72, 25],
    [45, 41, 63, 72, 93, 48, 30, 15, 45, 81, 18, 95, 30],
    [59, 84, 85, 85, 98, 52, 55, 40, 81, 92, 57, 100, 42],
    [77, 93, 105, 89, 102, 74, 81, 58, 91, 103, 92, 105, 104],
    [
        108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108,
    ],
    [
        111, 136, 138, 113, 180, 133, 111, 137, 131, 111, 121, 144, 118,
    ],
    [
        123, 153, 189, 126, 203, 138, 153, 149, 171, 180, 201, 156, 138,
    ],
    [
        148, 189, 200, 165, 208, 150, 167, 192, 193, 193, 206, 160, 163,
    ],
    [
        166, 199, 211, 200, 213, 212, 185, 201, 213, 197, 210, 182, 189,
    ],
];
const TMCC_DIFF_2: [[u16; 13]; 10] = [
    [13, 4, 10, 49, 16, 78, 23, 25, 36, 7, 31, 5, 34],
    [50, 7, 28, 61, 30, 82, 37, 63, 48, 25, 39, 10, 48],
    [70, 17, 44, 96, 37, 85, 51, 73, 55, 47, 47, 21, 54],
    [83, 51, 47, 99, 74, 98, 68, 80, 59, 60, 65, 44, 70],
    [87, 71, 54, 104, 83, 102, 105, 93, 86, 87, 72, 61, 101],
    [
        133, 144, 115, 139, 113, 142, 121, 112, 118, 157, 124, 186, 131,
    ],
    [
        171, 156, 133, 147, 118, 156, 158, 115, 136, 169, 138, 190, 145,
    ],
    [
        181, 163, 155, 155, 129, 162, 178, 125, 152, 204, 145, 193, 159,
    ],
    [
        188, 167, 168, 173, 152, 178, 191, 159, 155, 207, 182, 206, 176,
    ],
    [
        201, 194, 195, 180, 169, 209, 195, 179, 162, 212, 191, 210, 213,
    ],
];
const AC1_DIFF_3: [[u16; 13]; 8] = [
    [10, 20, 4, 98, 11, 76, 7, 61, 35, 8, 53, 74, 40],
    [28, 40, 89, 101, 101, 97, 89, 100, 79, 64, 83, 100, 89],
    [
        161, 182, 148, 118, 128, 112, 206, 119, 184, 115, 169, 143, 116,
    ],
    [
        191, 208, 197, 136, 148, 197, 209, 209, 205, 197, 208, 187, 172,
    ],
    [
        277, 251, 224, 269, 290, 256, 226, 236, 220, 314, 227, 292, 223,
    ],
    [
        316, 295, 280, 299, 316, 305, 244, 256, 305, 317, 317, 313, 305,
    ],
    [
        335, 400, 331, 385, 359, 332, 377, 398, 364, 334, 344, 328, 422,
    ],
    [
        425, 421, 413, 424, 403, 388, 407, 424, 413, 352, 364, 413, 425,
    ],
];
const AC2_DIFF_3: [[u16; 13]; 19] = [
    [3, 23, 13, 10, 28, 5, 25, 29, 3, 36, 3, 30, 72],
    [45, 63, 93, 30, 45, 18, 30, 41, 72, 48, 15, 81, 95],
    [59, 85, 98, 55, 81, 57, 42, 84, 85, 52, 40, 92, 100],
    [77, 105, 102, 81, 91, 92, 104, 93, 89, 74, 58, 103, 105],
    [
        108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108,
    ],
    [
        111, 138, 180, 111, 131, 121, 118, 136, 113, 133, 137, 111, 144,
    ],
    [
        123, 189, 203, 153, 171, 201, 138, 153, 126, 138, 149, 180, 156,
    ],
    [
        148, 200, 208, 167, 193, 206, 163, 189, 165, 150, 192, 193, 160,
    ],
    [
        166, 211, 213, 185, 213, 210, 189, 199, 200, 212, 201, 197, 182,
    ],
    [
        216, 216, 216, 216, 216, 216, 216, 216, 216, 216, 216, 216, 216,
    ],
    [
        245, 219, 252, 219, 246, 288, 219, 239, 229, 226, 244, 221, 241,
    ],
    [
        257, 288, 264, 231, 297, 311, 261, 279, 309, 246, 261, 234, 246,
    ],
    [
        300, 301, 268, 256, 308, 316, 275, 301, 314, 271, 297, 273, 258,
    ],
    [
        309, 305, 290, 274, 319, 321, 293, 321, 318, 297, 307, 308, 320,
    ],
    [
        324, 324, 324, 324, 324, 324, 324, 324, 324, 324, 324, 324, 324,
    ],
    [
        352, 329, 349, 353, 327, 360, 327, 354, 396, 327, 347, 337, 334,
    ],
    [
        369, 342, 354, 365, 396, 372, 339, 405, 419, 369, 387, 417, 354,
    ],
    [
        405, 381, 366, 408, 409, 376, 364, 416, 424, 383, 409, 422, 379,
    ],
    [
        415, 416, 428, 417, 413, 398, 382, 427, 429, 401, 429, 426, 405,
    ],
];
const TMCC_DIFF_3: [[u16; 13]; 20] = [
    [13, 10, 16, 23, 36, 31, 34, 4, 49, 78, 25, 7, 5],
    [50, 28, 30, 37, 48, 39, 48, 7, 61, 82, 63, 25, 10],
    [70, 44, 37, 51, 55, 47, 54, 17, 96, 85, 73, 47, 21],
    [83, 47, 74, 68, 59, 65, 70, 51, 99, 98, 80, 60, 44],
    [87, 54, 83, 105, 86, 72, 101, 71, 104, 102, 93, 87, 61],
    [
        133, 115, 113, 121, 118, 124, 131, 144, 139, 142, 112, 157, 186,
    ],
    [
        171, 133, 118, 158, 136, 138, 145, 156, 147, 156, 115, 169, 190,
    ],
    [
        181, 155, 129, 178, 152, 145, 159, 163, 155, 162, 125, 204, 193,
    ],
    [
        188, 168, 152, 191, 155, 182, 176, 167, 173, 178, 159, 207, 206,
    ],
    [
        201, 195, 169, 195, 162, 191, 213, 194, 180, 209, 179, 212, 210,
    ],
    [
        220, 265, 294, 241, 223, 221, 229, 226, 232, 239, 252, 247, 250,
    ],
    [
        223, 277, 298, 279, 241, 226, 266, 244, 246, 253, 264, 255, 264,
    ],
    [
        233, 312, 301, 289, 263, 237, 286, 260, 253, 267, 271, 263, 270,
    ],
    [
        267, 315, 314, 296, 276, 260, 299, 263, 290, 284, 275, 281, 286,
    ],
    [
        287, 320, 318, 309, 303, 277, 303, 270, 299, 321, 302, 288, 317,
    ],
    [
        360, 355, 358, 328, 373, 402, 349, 331, 329, 337, 334, 340, 347,
    ],
    [
        372, 363, 372, 331, 385, 406, 387, 349, 334, 374, 352, 354, 361,
    ],
    [
        379, 371, 378, 341, 420, 409, 397, 371, 345, 394, 368, 361, 375,
    ],
    [
        383, 389, 394, 375, 423, 422, 404, 384, 368, 407, 371, 398, 392,
    ],
    [
        410, 396, 425, 395, 428, 426, 417, 411, 385, 411, 378, 407, 429,
    ],
];
const AC1_COH_1: [[u16; 13]; 2] = [
    [10, 53, 61, 11, 20, 74, 35, 76, 4, 40, 8, 7, 98],
    [28, 83, 100, 101, 40, 100, 79, 97, 89, 89, 64, 89, 101],
];
const TMCC_COH_1: [[u16; 13]; 1] = [[70, 25, 17, 86, 44, 47, 49, 31, 83, 61, 85, 101, 23]];
const AC1_COH_2: [[u16; 13]; 4] = [
    [10, 61, 20, 35, 4, 8, 98, 53, 11, 74, 76, 40, 7],
    [28, 100, 40, 79, 89, 64, 101, 83, 101, 100, 97, 89, 89],
    [
        161, 119, 182, 184, 148, 115, 118, 169, 128, 143, 112, 116, 206,
    ],
    [
        191, 209, 208, 205, 197, 197, 136, 208, 148, 187, 197, 172, 209,
    ],
];
const TMCC_COH_2: [[u16; 13]; 2] = [
    [70, 17, 44, 49, 83, 85, 23, 25, 86, 47, 31, 61, 101],
    [
        133, 194, 155, 139, 169, 209, 178, 125, 152, 157, 191, 193, 131,
    ],
];
const AC1_COH_3: [[u16; 13]; 8] = [
    [10, 20, 4, 98, 11, 76, 7, 61, 35, 8, 53, 74, 40],
    [28, 40, 89, 101, 101, 97, 89, 100, 79, 64, 83, 100, 89],
    [
        161, 182, 148, 118, 128, 112, 206, 119, 184, 115, 169, 143, 116,
    ],
    [
        191, 208, 197, 136, 148, 197, 209, 209, 205, 197, 208, 187, 172,
    ],
    [
        277, 251, 224, 269, 290, 256, 226, 236, 220, 314, 227, 292, 223,
    ],
    [
        316, 295, 280, 299, 316, 305, 244, 256, 305, 317, 317, 313, 305,
    ],
    [
        335, 400, 331, 385, 359, 332, 377, 398, 364, 334, 344, 328, 422,
    ],
    [
        425, 421, 413, 424, 403, 388, 407, 424, 413, 352, 364, 413, 425,
    ],
];
const TMCC_COH_3: [[u16; 13]; 4] = [
    [70, 44, 83, 23, 86, 31, 101, 17, 49, 85, 25, 47, 61],
    [
        133, 155, 169, 178, 152, 191, 131, 194, 139, 209, 125, 157, 193,
    ],
    [
        233, 265, 301, 241, 263, 277, 286, 260, 299, 239, 302, 247, 317,
    ],
    [
        410, 355, 425, 341, 373, 409, 349, 371, 385, 394, 368, 407, 347,
    ],
];

// ARIB STD-B31 table 3-13: the carrier that data cell `i` of a rotated segment goes to.
const RANDOMISE_1: [u16; 96] = [
    80, 93, 63, 92, 94, 55, 17, 81, 6, 51, 9, 85, 89, 65, 52, 15, 73, 66, 46, 71, 12, 70, 18, 13,
    95, 34, 1, 38, 78, 59, 91, 64, 0, 28, 11, 4, 45, 35, 16, 7, 48, 22, 23, 77, 56, 19, 8, 36, 39,
    61, 21, 3, 26, 69, 67, 20, 74, 86, 72, 25, 31, 5, 49, 42, 54, 87, 43, 60, 29, 2, 76, 84, 83,
    40, 14, 79, 27, 57, 44, 37, 30, 68, 47, 88, 75, 41, 90, 10, 33, 32, 62, 50, 58, 82, 53, 24,
];
const RANDOMISE_2: [u16; 192] = [
    98, 35, 67, 116, 135, 17, 5, 93, 73, 168, 54, 143, 43, 74, 165, 48, 37, 69, 154, 150, 107, 76,
    176, 79, 175, 36, 28, 78, 47, 128, 94, 163, 184, 72, 142, 2, 86, 14, 130, 151, 114, 68, 46,
    183, 122, 112, 180, 42, 105, 97, 33, 134, 177, 84, 170, 45, 187, 38, 167, 10, 189, 51, 117,
    156, 161, 25, 89, 125, 139, 24, 19, 57, 71, 39, 77, 191, 88, 85, 0, 162, 181, 113, 140, 61, 75,
    82, 101, 174, 118, 20, 136, 3, 121, 190, 120, 92, 160, 52, 153, 127, 65, 60, 133, 147, 131, 87,
    22, 58, 100, 111, 141, 83, 49, 132, 12, 155, 146, 102, 164, 66, 1, 62, 178, 15, 182, 96, 80,
    119, 23, 6, 166, 56, 99, 123, 138, 137, 21, 145, 185, 18, 70, 129, 95, 90, 149, 109, 124, 50,
    11, 152, 4, 31, 172, 40, 13, 32, 55, 159, 41, 8, 7, 144, 16, 26, 173, 81, 44, 103, 64, 9, 30,
    157, 126, 179, 148, 63, 188, 171, 106, 104, 158, 115, 34, 186, 29, 108, 53, 91, 169, 110, 27,
    59,
];
const RANDOMISE_3: [u16; 384] = [
    62, 13, 371, 11, 285, 336, 365, 220, 226, 92, 56, 46, 120, 175, 298, 352, 172, 235, 53, 164,
    368, 187, 125, 82, 5, 45, 173, 258, 135, 182, 141, 273, 126, 264, 286, 88, 233, 61, 249, 367,
    310, 179, 155, 57, 123, 208, 14, 227, 100, 311, 205, 79, 184, 185, 328, 77, 115, 277, 112, 20,
    199, 178, 143, 152, 215, 204, 139, 234, 358, 192, 309, 183, 81, 129, 256, 314, 101, 43, 97,
    324, 142, 157, 90, 214, 102, 29, 303, 363, 261, 31, 22, 52, 305, 301, 293, 177, 116, 296, 85,
    196, 191, 114, 58, 198, 16, 167, 145, 119, 245, 113, 295, 193, 232, 17, 108, 283, 246, 64, 237,
    189, 128, 373, 302, 320, 239, 335, 356, 39, 347, 351, 73, 158, 276, 243, 99, 38, 287, 3, 330,
    153, 315, 117, 289, 213, 210, 149, 383, 337, 339, 151, 241, 321, 217, 30, 334, 161, 322, 49,
    176, 359, 12, 346, 60, 28, 229, 265, 288, 225, 382, 59, 181, 170, 319, 341, 86, 251, 133, 344,
    361, 109, 44, 369, 268, 257, 323, 55, 317, 381, 121, 360, 260, 275, 190, 19, 63, 18, 248, 9,
    240, 211, 150, 230, 332, 231, 71, 255, 350, 355, 83, 87, 154, 218, 138, 269, 348, 130, 160,
    278, 377, 216, 236, 308, 223, 254, 25, 98, 300, 201, 137, 219, 36, 325, 124, 66, 353, 169, 21,
    35, 107, 50, 106, 333, 326, 262, 252, 271, 263, 372, 136, 0, 366, 206, 159, 122, 188, 6, 284,
    96, 26, 200, 197, 186, 345, 340, 349, 103, 84, 228, 212, 2, 67, 318, 1, 74, 342, 166, 194, 33,
    68, 267, 111, 118, 140, 195, 105, 202, 291, 259, 23, 171, 65, 281, 24, 165, 8, 94, 222, 331,
    34, 238, 364, 376, 266, 89, 80, 253, 163, 280, 247, 4, 362, 379, 290, 279, 54, 78, 180, 72,
    316, 282, 131, 207, 343, 370, 306, 221, 132, 7, 148, 299, 168, 224, 48, 47, 357, 313, 75, 104,
    70, 147, 40, 110, 374, 69, 146, 37, 375, 354, 174, 41, 32, 304, 307, 312, 15, 272, 134, 242,
    203, 209, 380, 162, 297, 327, 10, 93, 42, 250, 156, 338, 292, 144, 378, 294, 329, 127, 270, 76,
    95, 91, 244, 274, 27, 51,
];

/// ARIB STD-B31 table 3-16: the pilot register at the first carrier of each segment, in frequency order, for modes 1, 2 and 3,
/// stage 1 first. It is the next 11 bits of the pilot sequence in reverse order.
pub const PILOT_INIT: [[[u8; 11]; 3]; 13] = [
    [
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    ], // segment 11
    [
        [1, 1, 0, 1, 1, 0, 0, 1, 1, 1, 1],
        [0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0],
        [1, 1, 0, 1, 1, 1, 0, 0, 1, 0, 1],
    ], // segment 9
    [
        [0, 1, 1, 0, 1, 0, 1, 1, 1, 1, 0],
        [1, 1, 0, 1, 1, 1, 0, 0, 1, 0, 1],
        [1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    ], // segment 7
    [
        [0, 1, 0, 0, 0, 1, 0, 1, 1, 1, 0],
        [1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0],
        [0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1],
    ], // segment 5
    [
        [1, 1, 0, 1, 1, 1, 0, 0, 1, 0, 1],
        [1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1],
    ], // segment 3
    [
        [0, 0, 1, 0, 1, 1, 1, 1, 0, 1, 0],
        [0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0],
        [1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0],
    ], // segment 1
    [
        [1, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0],
        [0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1],
        [0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1],
    ], // segment 0
    [
        [0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0],
        [0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0],
        [1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 1],
    ], // segment 2
    [
        [1, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1],
        [0, 1, 1, 0, 1, 0, 1, 0, 0, 1, 1],
    ], // segment 4
    [
        [1, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 1, 1, 1, 0, 0, 1],
        [1, 0, 1, 1, 1, 0, 1, 0, 0, 1, 0],
    ], // segment 6
    [
        [0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0],
        [1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0],
        [0, 1, 1, 0, 0, 0, 1, 0, 0, 1, 0],
    ], // segment 8
    [
        [1, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1],
        [0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1],
        [1, 1, 1, 1, 0, 1, 0, 0, 1, 0, 1],
    ], // segment 10
    [
        [0, 1, 1, 1, 0, 0, 0, 1, 0, 0, 1],
        [0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1],
        [0, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0],
    ], // segment 12
];

fn column(rows: &[[u16; 13]], pos: usize) -> Vec<u16> {
    rows.iter().map(|r| r[pos]).collect()
}

/// The TMCC and AC carriers of segment number `seg` in `mode`, for a differential (DQPSK) or a coherent segment. ARIB STD-B31
/// tables 3-14 and 3-15.
pub fn seg_table(mode: u8, seg: usize, differential: bool) -> SegTable {
    let p = pos_of_seg(seg);
    match (mode, differential) {
        (1, true) => SegTable {
            tmcc: column(&TMCC_DIFF_1, p),
            ac1: column(&AC1_DIFF_1, p),
            ac2: column(&AC2_DIFF_1, p),
        },
        (2, true) => SegTable {
            tmcc: column(&TMCC_DIFF_2, p),
            ac1: column(&AC1_DIFF_2, p),
            ac2: column(&AC2_DIFF_2, p),
        },
        (_, true) => SegTable {
            tmcc: column(&TMCC_DIFF_3, p),
            ac1: column(&AC1_DIFF_3, p),
            ac2: column(&AC2_DIFF_3, p),
        },
        (1, false) => SegTable {
            tmcc: column(&TMCC_COH_1, p),
            ac1: column(&AC1_COH_1, p),
            ac2: Vec::new(),
        },
        (2, false) => SegTable {
            tmcc: column(&TMCC_COH_2, p),
            ac1: column(&AC1_COH_2, p),
            ac2: Vec::new(),
        },
        (_, false) => SegTable {
            tmcc: column(&TMCC_COH_3, p),
            ac1: column(&AC1_COH_3, p),
            ac2: Vec::new(),
        },
    }
}

/// The intra-segment carrier randomising of a mode: entry `i` is the data carrier that rotated cell `i` goes to. ARIB STD-B31
/// table 3-13.
pub fn randomiser(mode: u8) -> &'static [u16] {
    match mode {
        1 => &RANDOMISE_1,
        2 => &RANDOMISE_2,
        _ => &RANDOMISE_3,
    }
}

/// Whether carrier `k` of a synchronous segment is a scattered pilot in symbol `sym` of the frame. ARIB STD-B31 clause 3.13.1.
#[inline]
pub fn is_sp(k: usize, sym: usize) -> bool {
    k % 12 == 3 * (sym % 4)
}

/// Bit-interleaver delays (in cells) of each bit position of a cell with `bits` bits (ARIB STD-B31 Table 3-6).
pub fn bit_delays(bits: usize) -> &'static [u32] {
    match bits {
        2 => &[120, 0],
        4 => &[120, 80, 40, 0],
        _ => &[120, 96, 72, 48, 24, 0],
    }
}

/// The time-interleaver delay adjustment of a layer (in symbols): the time interleaver's delay `95 I` plus this is a whole number
/// of frames (ARIB STD-B31 Table 3-12).
pub fn ti_adjust(i_len: usize) -> usize {
    (95 * i_len).div_ceil(204) * 204 - 95 * i_len
}

/// The time-interleaving length I of a mode and a length code (0..3).
pub fn ti_length(mode: u8, code: u8) -> usize {
    let base = [0usize, 4, 8, 16];
    base[code.min(3) as usize] >> (mode - 1)
}

/// The time-interleaver delays (in symbols) of each data cell of a layer of `cells` cells per symbol, `d` data carriers per
/// segment: carrier `c` of a segment is delayed by `I m` with `m = (5 c) mod 96` when sending, `I (95 - m)` when receiving, plus
/// the adjustment to whole frames when sending. ARIB STD-B31 clause 3.10.
pub fn ti_delays(cells: usize, d: usize, i_len: usize, receive: bool) -> Vec<u32> {
    let adj = ti_adjust(i_len);
    (0..cells)
        .map(|c| {
            let m = (5 * (c % d)) % 96;
            if receive {
                (i_len * (95 - m)) as u32
            } else {
                (i_len * m + adj) as u32
            }
        })
        .collect()
}

/// The frame layout of one configuration: which segment is differential, the data carriers of each segment in each of the four
/// pilot phases, and the frequency-interleaving map.
#[derive(Clone, Debug)]
pub struct Layout {
    /// Mode 1..3.
    pub mode: u8,
    /// Carriers per segment.
    pub ls: usize,
    /// Data carriers per segment.
    pub d: usize,
    /// Carriers in the band.
    pub nc: usize,
    /// The band.
    pub band_of: Band,
    /// FFT size.
    pub n_fft: usize,
    /// Per segment number: differential (DQPSK).
    pub diff: [bool; 13],
    /// Per segment number: the layer (0 A, 1 B, 2 C).
    pub layer: [u8; 13],
    /// Per segment number: TMCC and AC carriers.
    pub tables: Vec<SegTable>,
    /// `data[s * 4 + phase]`: the data carriers (inside the segment) of segment number `s` in pilot phase `phase`.
    pub data: Vec<Vec<u16>>,
    /// Frequency interleaving: the cell at index `j` (segment-number order, `s * d + i`) goes to data position `fi[j]`.
    pub fi: Vec<u32>,
    /// The pilot sequence across the band.
    pub prbs: Vec<u8>,
}

impl Layout {
    /// The layout of a configuration in `band` (its layers must fill the band: [`Config::within`]).
    pub fn new(mode: u8, cfg: &Config, band: Band) -> Layout {
        let ls = seg_carriers(mode);
        let d = 96usize << (mode - 1);
        let ns = band.ns;
        let nc = band.carriers(mode);
        let mut diff = [false; 13];
        let mut layer = [0u8; 13];
        let mut s = 0usize;
        for (li, l) in cfg.layers.iter().enumerate() {
            if !l.used() {
                continue;
            }
            for _ in 0..l.segments {
                if s < ns {
                    diff[s] = l.modulation == 0;
                    layer[s] = li as u8;
                }
                s += 1;
            }
        }
        let tables: Vec<SegTable> = (0..ns).map(|s| seg_table(mode, s, diff[s])).collect();
        let mut data = Vec::with_capacity(4 * ns);
        for s in 0..ns {
            let t = &tables[s];
            for phase in 0..4 {
                let mut special = vec![false; ls];
                for &k in t.tmcc.iter().chain(&t.ac1).chain(&t.ac2) {
                    special[k as usize] = true;
                }
                let v: Vec<u16> = (0..ls)
                    .filter(|&k| !special[k] && if diff[s] { k != 0 } else { !is_sp(k, phase) })
                    .map(|k| k as u16)
                    .collect();
                debug_assert_eq!(v.len(), d);
                data.push(v);
            }
        }
        // Groups: the partial-reception segment alone, the differential segments, the synchronous segments.
        let mut groups: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        if cfg.partial {
            groups.push((0, 1));
            start = 1;
        }
        let nd = (start..ns).take_while(|&s| diff[s]).count();
        if nd > 0 {
            groups.push((start, nd));
        }
        if start + nd < ns {
            groups.push((start + nd, ns - start - nd));
        }
        let rnd = randomiser(mode);
        let mut fi = vec![0u32; ns * d];
        for &(s0, n) in &groups {
            for j in 0..n {
                for i in 0..d {
                    // Between the segments of the group (figure 3-26): cell b = d j + i goes to segment b mod n, place b div n.
                    let b = d * j + i;
                    let (seg, i2) = (s0 + b % n, b / n);
                    // Inside the segment (figure 3-27): the rotation puts cell (r + seg) mod d at place r, then table 3-13
                    // moves place r to its carrier.
                    let r = (i2 + d - seg % d) % d;
                    fi[(s0 + j) * d + i] = (seg * d + rnd[r] as usize) as u32;
                }
            }
        }
        // The pilot sequence runs from carrier 0 of the lowest of the 13 places; a narrow band takes its own stretch of it.
        let p0 = band.first_pos() * ls;
        let mut prbs = pilot_prbs(13 * ls + 1)[p0..p0 + nc].to_vec();
        match band.edge {
            EdgePilot::Sequence => {}
            EdgePilot::Plus => prbs[nc - 1] = 0,
            EdgePilot::Minus => prbs[nc - 1] = 1,
        }
        Layout {
            mode,
            ls,
            d,
            nc,
            band_of: band,
            n_fft: band.fft_size(mode),
            diff,
            layer,
            tables,
            data,
            fi,
            prbs,
        }
    }
    /// Segments in the band.
    #[inline]
    pub fn ns(&self) -> usize {
        self.band_of.ns
    }
    /// The position of segment number `s` in the band, from 0 at the lowest.
    #[inline]
    pub fn pos(&self, s: usize) -> usize {
        pos_of_seg(s) - self.band_of.first_pos()
    }
    /// The band index of carrier `k` of segment number `s`.
    #[inline]
    pub fn band(&self, s: usize, k: usize) -> usize {
        self.pos(s) * self.ls + k
    }
    /// The pilot value (±4/3) at band carrier `c`.
    #[inline]
    pub fn pilot(&self, c: usize) -> f32 {
        if self.prbs[c] == 0 {
            PILOT
        } else {
            -PILOT
        }
    }
    /// The continual pilot at the top of the band.
    pub fn top_pilot(&self) -> f32 {
        self.pilot(self.nc - 1)
    }
    /// Places one symbol of interleaved data cells (`ns d`, segment-number order, already frequency-interleaved) into the band
    /// (`nc` carriers). `sym` the symbol's index in the frame.
    pub fn place(&self, cells: &[Complex32], sym: usize, band: &mut [Complex32]) {
        for s in 0..self.ns() {
            let base = self.pos(s) * self.ls;
            let list = &self.data[s * 4 + if self.diff[s] { 0 } else { sym % 4 }];
            for (i, &k) in list.iter().enumerate() {
                band[base + k as usize] = cells[s * self.d + i];
            }
        }
    }
    /// Reads one symbol's data cells out of the band (the inverse of [`place`](Self::place)), into segment-number order.
    pub fn take<T: Copy>(&self, band: &[T], sym: usize, out: &mut [T]) {
        for s in 0..self.ns() {
            let base = self.pos(s) * self.ls;
            let list = &self.data[s * 4 + if self.diff[s] { 0 } else { sym % 4 }];
            for (i, &k) in list.iter().enumerate() {
                out[s * self.d + i] = band[base + k as usize];
            }
        }
    }
    /// The frequency interleaving of one symbol (`input` in layer order, `out` in data-position order).
    pub fn interleave<T: Copy>(&self, input: &[T], out: &mut [T]) {
        for (j, &p) in self.fi.iter().enumerate() {
            out[p as usize] = input[j];
        }
    }
    /// Its inverse.
    pub fn deinterleave<T: Copy>(&self, input: &[T], out: &mut [T]) {
        for (j, &p) in self.fi.iter().enumerate() {
            out[j] = input[p as usize];
        }
    }
    /// The first segment number of each layer and its segment count.
    pub fn layer_span(cfg: &Config, layer: usize) -> (usize, usize) {
        let mut s = 0usize;
        for (li, l) in cfg.layers.iter().enumerate() {
            let n = if l.used() { l.segments as usize } else { 0 };
            if li == layer {
                return (s, n);
            }
            s += n;
        }
        (s, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::super::tmcc::LayerParams;
    use super::*;

    fn cfg(partial: bool, l: [(u8, u8, u8); 3]) -> Config {
        let mut c = Config {
            partial,
            layers: [LayerParams::UNUSED; 3],
        };
        for (i, &(m, r, n)) in l.iter().enumerate() {
            if n > 0 {
                c.layers[i] = LayerParams {
                    modulation: m,
                    code_rate: r,
                    interleave: 1,
                    segments: n,
                };
            }
        }
        c
    }

    #[test]
    fn layout_groups_and_refusal() {
        // Mode 3: A 1 seg QPSK partial, B 3 seg DQPSK, C 9 seg 64QAM.
        let c = cfg(true, [(1, 1, 1), (0, 1, 3), (3, 2, 9)]);
        assert!(c.valid());
        let l = Layout::new(3, &c, Band::FULL);
        assert_eq!(l.layer[0], 0);
        assert!(l.diff[1] && l.diff[2] && l.diff[3] && !l.diff[4]);
        let d = l.d;
        // Segment 0 stays in segment 0. Segments 1..3 interleave among themselves. 4..12 among themselves.
        for j in 0..d {
            assert_eq!(l.fi[j] as usize / d, 0);
        }
        for j in d..4 * d {
            assert!((1..4).contains(&(l.fi[j] as usize / d)));
        }
        for j in 4 * d..13 * d {
            assert!((4..13).contains(&(l.fi[j] as usize / d)));
        }
        // The map is a permutation.
        let mut seen = vec![false; 13 * d];
        for &p in &l.fi {
            assert!(!seen[p as usize]);
            seen[p as usize] = true;
        }
        // A DQPSK layer after 16QAM is refused.
        assert!(!cfg(false, [(2, 1, 4), (0, 1, 9), (7, 7, 0)]).valid());
    }

    #[test]
    fn carrier_tables_are_those_of_the_standard() {
        // Table 3-13: the first and last entries of each mode.
        assert_eq!((randomiser(1)[0], randomiser(1)[95]), (80, 24));
        assert_eq!((randomiser(2)[0], randomiser(2)[191]), (98, 59));
        assert_eq!((randomiser(3)[0], randomiser(3)[383]), (62, 51));
        // Tables 3-14 and 3-15: segment 0 of mode 1 and segment 11 of mode 3.
        assert_eq!(seg_table(1, 0, false).tmcc, [49]);
        assert_eq!(seg_table(1, 0, false).ac1, [35, 79]);
        assert_eq!(seg_table(1, 0, true).tmcc, [49, 61, 96, 99, 104]);
        assert_eq!(seg_table(3, 11, true).ac2[18], 415);
        assert_eq!(seg_table(3, 12, false).tmcc, [61, 193, 317, 347]);
        for mode in 1..=3u8 {
            let d = 96usize << (mode - 1);
            let mut r = randomiser(mode).to_vec();
            r.sort();
            assert!(
                r.iter().enumerate().all(|(i, &v)| v as usize == i),
                "mode {mode}"
            );
            let m = 1 << (mode - 1);
            let ls = seg_carriers(mode);
            for seg in 0..13 {
                // AC1 is at the same carriers in both kinds of segment.
                assert_eq!(
                    seg_table(mode, seg, true).ac1,
                    seg_table(mode, seg, false).ac1
                );
                for diff in [false, true] {
                    let t = seg_table(mode, seg, diff);
                    assert_eq!(t.tmcc.len(), if diff { 5 * m } else { m });
                    assert_eq!(t.ac1.len(), 2 * m);
                    assert_eq!(t.ac2.len(), if diff { 5 * m - 1 } else { 0 });
                    let mut all: Vec<u16> =
                        t.tmcc.iter().chain(&t.ac1).chain(&t.ac2).copied().collect();
                    for &k in &all {
                        assert!((k as usize) < ls && k != 0);
                        // A coherent segment never puts them where a scattered pilot goes.
                        assert!(diff || k % 3 != 0, "mode {mode} segment {seg}: {k}");
                    }
                    all.sort();
                    all.dedup();
                    assert_eq!(all.len(), t.tmcc.len() + t.ac1.len() + t.ac2.len());
                }
            }
            // Every segment has exactly the data carriers of its mode in each phase.
            let c = cfg(false, [(3, 2, 13), (7, 7, 0), (7, 7, 0)]);
            let l = Layout::new(mode, &c, Band::FULL);
            assert!(l.data.iter().all(|v| v.len() == d));
        }
        assert_eq!(ti_adjust(1), 109);
        assert_eq!(ti_adjust(4), 28);
        assert_eq!(ti_length(3, 2), 2);
        assert_eq!(ti_length(1, 3), 16);
    }

    #[test]
    fn frequency_interleaving_follows_figures_3_26_and_3_27() {
        // Mode 1, 13 coherent segments: cell 13 of segment 0 goes to place 1 of segment 0 (figure 3-26), which table 3-13 puts
        // on carrier 93. Cell 1 goes to place 0 of segment 1, which the rotation by 1 moves to place 95, carrier 24.
        let c = cfg(false, [(3, 2, 13), (7, 7, 0), (7, 7, 0)]);
        let l = Layout::new(1, &c, Band::FULL);
        assert_eq!(l.fi[13], 93);
        assert_eq!(l.fi[1], 96 + 24);
        // The partial-reception segment is interleaved alone and not rotated.
        let c = cfg(true, [(1, 1, 1), (3, 2, 12), (7, 7, 0)]);
        let l = Layout::new(1, &c, Band::FULL);
        assert_eq!(l.fi[0], 80);
        assert_eq!(l.fi[1], 93);
    }

    #[test]
    fn the_top_pilot_continues_the_sequence() {
        // The continual pilot above segment 12: −4/3 in mode 1, +4/3 in modes 2 and 3.
        let c = cfg(false, [(3, 2, 13), (7, 7, 0), (7, 7, 0)]);
        for (mode, v) in [(1u8, -PILOT), (2, PILOT), (3, PILOT)] {
            assert_eq!(Layout::new(mode, &c, Band::FULL).top_pilot(), v);
        }
    }

    #[test]
    fn the_pilot_sequence_matches_table_3_16() {
        assert_eq!(PILOTS, sdrkit_util_bits::seq::DVBT_PILOTS);
        for mode in 1..=3u8 {
            let ls = seg_carriers(mode);
            let w = pilot_prbs(13 * ls + 1);
            for (p, row) in PILOT_INIT.iter().enumerate() {
                let next: Vec<u8> = w[p * ls..p * ls + 11].iter().rev().copied().collect();
                assert_eq!(next, row[mode as usize - 1], "mode {mode} position {p}");
            }
        }
    }
}
