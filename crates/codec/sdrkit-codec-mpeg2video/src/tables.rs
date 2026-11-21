//! The tables of ISO/IEC 13818-2: the variable-length codes of Annex B, the two scans, the default quantiser matrices and
//! the quantiser scales. The codes are written as in the standard, first bit first; spaces have no meaning.

use std::sync::OnceLock;

/// Table B-1, macroblock_address_increment: code and increment.
pub const ADDRESS_INCREMENT: &[(&str, u8)] = &[
    ("1", 1),
    ("011", 2),
    ("010", 3),
    ("0011", 4),
    ("0010", 5),
    ("0001 1", 6),
    ("0001 0", 7),
    ("0000 111", 8),
    ("0000 110", 9),
    ("0000 1011", 10),
    ("0000 1010", 11),
    ("0000 1001", 12),
    ("0000 1000", 13),
    ("0000 0111", 14),
    ("0000 0110", 15),
    ("0000 0101 11", 16),
    ("0000 0101 10", 17),
    ("0000 0101 01", 18),
    ("0000 0101 00", 19),
    ("0000 0100 11", 20),
    ("0000 0100 10", 21),
    ("0000 0100 011", 22),
    ("0000 0100 010", 23),
    ("0000 0100 001", 24),
    ("0000 0100 000", 25),
    ("0000 0011 111", 26),
    ("0000 0011 110", 27),
    ("0000 0011 101", 28),
    ("0000 0011 100", 29),
    ("0000 0011 011", 30),
    ("0000 0011 010", 31),
    ("0000 0011 001", 32),
    ("0000 0011 000", 33),
    ("0000 0001 000", ADDRESS_ESCAPE),
];

/// The value of the escape code in [`ADDRESS_INCREMENT`]: it adds 33 to the increment that follows.
pub const ADDRESS_ESCAPE: u8 = 34;

/// macroblock_type flag: a quantiser scale code follows.
pub const MB_QUANT: u8 = 1;
/// macroblock_type flag: forward motion vectors follow.
pub const MB_FORWARD: u8 = 2;
/// macroblock_type flag: backward motion vectors follow.
pub const MB_BACKWARD: u8 = 4;
/// macroblock_type flag: a coded block pattern follows.
pub const MB_PATTERN: u8 = 8;
/// macroblock_type flag: the macroblock is intra coded.
pub const MB_INTRA: u8 = 16;

/// Table B-2, macroblock_type in I pictures: code and flags.
pub const TYPE_I: &[(&str, u8)] = &[("1", MB_INTRA), ("01", MB_INTRA | MB_QUANT)];

/// Table B-3, macroblock_type in P pictures: code and flags.
pub const TYPE_P: &[(&str, u8)] = &[
    ("1", MB_FORWARD | MB_PATTERN),
    ("01", MB_PATTERN),
    ("001", MB_FORWARD),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FORWARD | MB_PATTERN),
    ("0000 1", MB_QUANT | MB_PATTERN),
    ("0000 01", MB_INTRA | MB_QUANT),
];

/// Table B-4, macroblock_type in B pictures: code and flags.
pub const TYPE_B: &[(&str, u8)] = &[
    ("10", MB_FORWARD | MB_BACKWARD),
    ("11", MB_FORWARD | MB_BACKWARD | MB_PATTERN),
    ("010", MB_BACKWARD),
    ("011", MB_BACKWARD | MB_PATTERN),
    ("0010", MB_FORWARD),
    ("0011", MB_FORWARD | MB_PATTERN),
    ("0001 1", MB_INTRA),
    ("0001 0", MB_QUANT | MB_FORWARD | MB_BACKWARD | MB_PATTERN),
    ("0000 11", MB_QUANT | MB_FORWARD | MB_PATTERN),
    ("0000 10", MB_QUANT | MB_BACKWARD | MB_PATTERN),
    ("0000 01", MB_INTRA | MB_QUANT),
];

/// Table B-9, coded_block_pattern: code and pattern (bit 5 is the first luminance block, bit 0 is Cr).
pub const BLOCK_PATTERN: &[(&str, u8)] = &[
    ("111", 60),
    ("1101", 4),
    ("1100", 8),
    ("1011", 16),
    ("1010", 32),
    ("1001 1", 12),
    ("1001 0", 48),
    ("1000 1", 20),
    ("1000 0", 40),
    ("0111 1", 28),
    ("0111 0", 44),
    ("0110 1", 52),
    ("0110 0", 56),
    ("0101 1", 1),
    ("0101 0", 61),
    ("0100 1", 2),
    ("0100 0", 62),
    ("0011 11", 24),
    ("0011 10", 36),
    ("0011 01", 3),
    ("0011 00", 63),
    ("0010 111", 5),
    ("0010 110", 9),
    ("0010 101", 17),
    ("0010 100", 33),
    ("0010 011", 6),
    ("0010 010", 10),
    ("0010 001", 18),
    ("0010 000", 34),
    ("0001 1111", 7),
    ("0001 1110", 11),
    ("0001 1101", 19),
    ("0001 1100", 35),
    ("0001 1011", 13),
    ("0001 1010", 49),
    ("0001 1001", 21),
    ("0001 1000", 41),
    ("0001 0111", 14),
    ("0001 0110", 50),
    ("0001 0101", 22),
    ("0001 0100", 42),
    ("0001 0011", 15),
    ("0001 0010", 51),
    ("0001 0001", 23),
    ("0001 0000", 43),
    ("0000 1111", 25),
    ("0000 1110", 37),
    ("0000 1101", 26),
    ("0000 1100", 38),
    ("0000 1011", 29),
    ("0000 1010", 45),
    ("0000 1001", 53),
    ("0000 1000", 57),
    ("0000 0111", 30),
    ("0000 0110", 46),
    ("0000 0101", 54),
    ("0000 0100", 58),
    ("0000 0011 1", 31),
    ("0000 0011 0", 47),
    ("0000 0010 1", 55),
    ("0000 0010 0", 59),
    ("0000 0001 1", 27),
    ("0000 0001 0", 39),
    ("0000 0000 1", 0),
];

/// Table B-10, motion_code: code without the sign bit and magnitude. A sign bit follows every magnitude but 0
/// (0 is positive).
pub const MOTION_CODE: &[(&str, u8)] = &[
    ("1", 0),
    ("01", 1),
    ("001", 2),
    ("0001", 3),
    ("0000 11", 4),
    ("0000 101", 5),
    ("0000 100", 6),
    ("0000 011", 7),
    ("0000 0101 1", 8),
    ("0000 0101 0", 9),
    ("0000 0100 1", 10),
    ("0000 0100 01", 11),
    ("0000 0100 00", 12),
    ("0000 0011 11", 13),
    ("0000 0011 10", 14),
    ("0000 0011 01", 15),
    ("0000 0011 00", 16),
];

/// Table B-12, dct_dc_size_luminance: code and size.
pub const DC_SIZE_LUMA: &[(&str, u8)] = &[
    ("100", 0),
    ("00", 1),
    ("01", 2),
    ("101", 3),
    ("110", 4),
    ("1110", 5),
    ("1111 0", 6),
    ("1111 10", 7),
    ("1111 110", 8),
    ("1111 1110", 9),
    ("1111 1111 0", 10),
    ("1111 1111 1", 11),
];

/// Table B-13, dct_dc_size_chrominance: code and size.
pub const DC_SIZE_CHROMA: &[(&str, u8)] = &[
    ("00", 0),
    ("01", 1),
    ("10", 2),
    ("110", 3),
    ("1110", 4),
    ("1111 0", 5),
    ("1111 10", 6),
    ("1111 110", 7),
    ("1111 1110", 8),
    ("1111 1111 0", 9),
    ("1111 1111 10", 10),
    ("1111 1111 11", 11),
];

/// The code of an escape in both coefficient tables: a 6-bit run and a 12-bit level in two's complement follow.
pub const COEFF_ESCAPE: &str = "0000 01";
/// The end-of-block code of table B-14.
pub const COEFF_END_ZERO: &str = "10";
/// The end-of-block code of table B-15.
pub const COEFF_END_ONE: &str = "0110";

/// Table B-14, DCT coefficients table zero: code without the sign bit, run and level. The first coefficient of a
/// non-intra block has the code `1` for run 0, level 1.
pub const COEFF_ZERO: &[(&str, u8, u8)] = &[
    ("11", 0, 1),
    ("011", 1, 1),
    ("0100", 0, 2),
    ("0101", 2, 1),
    ("0010 1", 0, 3),
    ("0011 1", 3, 1),
    ("0011 0", 4, 1),
    ("0001 10", 1, 2),
    ("0001 11", 5, 1),
    ("0001 01", 6, 1),
    ("0001 00", 7, 1),
    ("0000 110", 0, 4),
    ("0000 100", 2, 2),
    ("0000 111", 8, 1),
    ("0000 101", 9, 1),
    ("0010 0110", 0, 5),
    ("0010 0001", 0, 6),
    ("0010 0101", 1, 3),
    ("0010 0100", 3, 2),
    ("0010 0111", 10, 1),
    ("0010 0011", 11, 1),
    ("0010 0010", 12, 1),
    ("0010 0000", 13, 1),
    ("0000 0010 10", 0, 7),
    ("0000 0011 00", 1, 4),
    ("0000 0010 11", 2, 3),
    ("0000 0011 11", 4, 2),
    ("0000 0010 01", 5, 2),
    ("0000 0011 10", 14, 1),
    ("0000 0011 01", 15, 1),
    ("0000 0010 00", 16, 1),
    ("0000 0001 1101", 0, 8),
    ("0000 0001 1000", 0, 9),
    ("0000 0001 0011", 0, 10),
    ("0000 0001 0000", 0, 11),
    ("0000 0001 1011", 1, 5),
    ("0000 0001 0100", 2, 4),
    ("0000 0001 1100", 3, 3),
    ("0000 0001 0010", 4, 3),
    ("0000 0001 1110", 6, 2),
    ("0000 0001 0101", 7, 2),
    ("0000 0001 0001", 8, 2),
    ("0000 0001 1111", 17, 1),
    ("0000 0001 1010", 18, 1),
    ("0000 0001 1001", 19, 1),
    ("0000 0001 0111", 20, 1),
    ("0000 0001 0110", 21, 1),
    ("0000 0000 1101 0", 0, 12),
    ("0000 0000 1100 1", 0, 13),
    ("0000 0000 1100 0", 0, 14),
    ("0000 0000 1011 1", 0, 15),
    ("0000 0000 1011 0", 1, 6),
    ("0000 0000 1010 1", 1, 7),
    ("0000 0000 1010 0", 2, 5),
    ("0000 0000 1001 1", 3, 4),
    ("0000 0000 1001 0", 5, 3),
    ("0000 0000 1000 1", 9, 2),
    ("0000 0000 1000 0", 10, 2),
    ("0000 0000 1111 1", 22, 1),
    ("0000 0000 1111 0", 23, 1),
    ("0000 0000 1110 1", 24, 1),
    ("0000 0000 1110 0", 25, 1),
    ("0000 0000 1101 1", 26, 1),
    ("0000 0000 0111 11", 0, 16),
    ("0000 0000 0111 10", 0, 17),
    ("0000 0000 0111 01", 0, 18),
    ("0000 0000 0111 00", 0, 19),
    ("0000 0000 0110 11", 0, 20),
    ("0000 0000 0110 10", 0, 21),
    ("0000 0000 0110 01", 0, 22),
    ("0000 0000 0110 00", 0, 23),
    ("0000 0000 0101 11", 0, 24),
    ("0000 0000 0101 10", 0, 25),
    ("0000 0000 0101 01", 0, 26),
    ("0000 0000 0101 00", 0, 27),
    ("0000 0000 0100 11", 0, 28),
    ("0000 0000 0100 10", 0, 29),
    ("0000 0000 0100 01", 0, 30),
    ("0000 0000 0100 00", 0, 31),
    ("0000 0000 0011 000", 0, 32),
    ("0000 0000 0010 111", 0, 33),
    ("0000 0000 0010 110", 0, 34),
    ("0000 0000 0010 101", 0, 35),
    ("0000 0000 0010 100", 0, 36),
    ("0000 0000 0010 011", 0, 37),
    ("0000 0000 0010 010", 0, 38),
    ("0000 0000 0010 001", 0, 39),
    ("0000 0000 0010 000", 0, 40),
    ("0000 0000 0011 111", 1, 8),
    ("0000 0000 0011 110", 1, 9),
    ("0000 0000 0011 101", 1, 10),
    ("0000 0000 0011 100", 1, 11),
    ("0000 0000 0011 011", 1, 12),
    ("0000 0000 0011 010", 1, 13),
    ("0000 0000 0011 001", 1, 14),
    ("0000 0000 0001 0011", 1, 15),
    ("0000 0000 0001 0010", 1, 16),
    ("0000 0000 0001 0001", 1, 17),
    ("0000 0000 0001 0000", 1, 18),
    ("0000 0000 0001 0100", 6, 3),
    ("0000 0000 0001 1010", 11, 2),
    ("0000 0000 0001 1001", 12, 2),
    ("0000 0000 0001 1000", 13, 2),
    ("0000 0000 0001 0111", 14, 2),
    ("0000 0000 0001 0110", 15, 2),
    ("0000 0000 0001 0101", 16, 2),
    ("0000 0000 0001 1111", 27, 1),
    ("0000 0000 0001 1110", 28, 1),
    ("0000 0000 0001 1101", 29, 1),
    ("0000 0000 0001 1100", 30, 1),
    ("0000 0000 0001 1011", 31, 1),
];

/// Table B-15, DCT coefficients table one, for intra blocks when intra_vlc_format is 1: the codes that differ from
/// table B-14. The codes of 14 to 16 bits and the 12- and 13-bit codes listed in [`COEFF_SHARED`] are those of B-14.
pub const COEFF_ONE_SHORT: &[(&str, u8, u8)] = &[
    ("10", 0, 1),
    ("010", 1, 1),
    ("110", 0, 2),
    ("0010 1", 2, 1),
    ("0111", 0, 3),
    ("0011 1", 3, 1),
    ("0001 10", 4, 1),
    ("0011 0", 1, 2),
    ("0001 11", 5, 1),
    ("0000 110", 6, 1),
    ("0000 100", 7, 1),
    ("1110 0", 0, 4),
    ("0000 111", 2, 2),
    ("0000 101", 8, 1),
    ("1111 000", 9, 1),
    ("1110 1", 0, 5),
    ("0001 01", 0, 6),
    ("1111 001", 1, 3),
    ("0010 0110", 3, 2),
    ("1111 010", 10, 1),
    ("0010 0001", 11, 1),
    ("0010 0101", 12, 1),
    ("0010 0100", 13, 1),
    ("0001 00", 0, 7),
    ("0010 0111", 1, 4),
    ("1111 1100", 2, 3),
    ("1111 1101", 4, 2),
    ("0000 0010 0", 5, 2),
    ("0000 0010 1", 14, 1),
    ("0000 0011 1", 15, 1),
    ("0000 0011 01", 16, 1),
    ("1111 011", 0, 8),
    ("1111 100", 0, 9),
    ("0010 0011", 0, 10),
    ("0010 0010", 0, 11),
    ("0010 0000", 1, 5),
    ("0000 0011 00", 2, 4),
    ("1111 1010", 0, 12),
    ("1111 1011", 0, 13),
    ("1111 1110", 0, 14),
    ("1111 1111", 0, 15),
];

/// The (run, level) pairs of 12 and 13 bits that table B-15 shares with table B-14.
pub const COEFF_SHARED: &[(u8, u8)] = &[
    (3, 3),
    (4, 3),
    (6, 2),
    (7, 2),
    (8, 2),
    (17, 1),
    (18, 1),
    (19, 1),
    (20, 1),
    (21, 1),
    (1, 6),
    (1, 7),
    (2, 5),
    (3, 4),
    (5, 3),
    (9, 2),
    (10, 2),
    (22, 1),
    (23, 1),
    (24, 1),
    (25, 1),
    (26, 1),
];

/// The whole of table B-15: code without the sign bit, run and level.
pub fn coeff_one() -> Vec<(&'static str, u8, u8)> {
    let mut v = COEFF_ONE_SHORT.to_vec();
    for &(code, run, level) in COEFF_ZERO {
        let bits = code.bytes().filter(|b| *b != b' ').count();
        if bits >= 14 || (bits >= 12 && COEFF_SHARED.contains(&(run, level))) {
            v.push((code, run, level));
        }
    }
    v
}

/// The zigzag scan: the place in the block, counted along the rows, of each coefficient in the order sent.
pub const ZIGZAG: [u8; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// The alternate scan, in the form of [`ZIGZAG`].
pub const ALTERNATE: [u8; 64] = [
    0, 8, 16, 24, 1, 9, 2, 10, 17, 25, 32, 40, 48, 56, 57, 49, 41, 33, 26, 18, 3, 11, 4, 12, 19,
    27, 34, 42, 50, 58, 35, 43, 51, 59, 20, 28, 5, 13, 6, 14, 21, 29, 36, 44, 52, 60, 37, 45, 53,
    61, 22, 30, 7, 15, 23, 31, 38, 46, 54, 62, 39, 47, 55, 63,
];

/// The default intra quantiser matrix, along the rows.
pub const DEFAULT_INTRA_MATRIX: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34, 38,
    22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27, 29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40, 48, 58,
    26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

/// The quantiser scale of each quantiser_scale_code when q_scale_type is 1. With q_scale_type 0 it is twice the code.
pub const NON_LINEAR_SCALE: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64,
    72, 80, 88, 96, 104, 112,
];

/// The quantiser scale of `code` (1 to 31).
pub fn quantiser_scale(non_linear: bool, code: u8) -> i32 {
    if non_linear {
        NON_LINEAR_SCALE[(code & 31) as usize] as i32
    } else {
        2 * (code & 31) as i32
    }
}

/// The bits of a code written as in the tables, and their count.
pub fn code_bits(code: &str) -> (u32, u32) {
    code.bytes()
        .filter(|b| *b != b' ')
        .fold((0, 0), |(v, n), b| ((v << 1) | (b == b'1') as u32, n + 1))
}

/// One entry of a decoding table. `len` 0 marks bits that are no code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Entry {
    /// The length of the code in bits.
    pub len: u8,
    /// The value, or the run of a coefficient.
    pub a: u8,
    /// The level of a coefficient ([`END`] and [`ESCAPE`] for the two special codes).
    pub b: i16,
}

/// `Entry::b` of an end-of-block code.
pub(crate) const END: i16 = 0;
/// `Entry::b` of an escape code.
pub(crate) const ESCAPE: i16 = -1;
const LINK: u8 = 255;

/// A decoding table for codes of up to 16 bits: one step for codes of up to 8 bits, two for longer ones.
pub(crate) struct Vlc {
    first: [Entry; 256],
    more: Vec<[Entry; 256]>,
}

impl Vlc {
    fn build(codes: impl Iterator<Item = (&'static str, u8, i16)>) -> Vlc {
        let mut t = Vlc {
            first: [Entry::default(); 256],
            more: Vec::new(),
        };
        for (code, a, b) in codes {
            let (bits, n) = code_bits(code);
            let e = Entry { len: n as u8, a, b };
            if n <= 8 {
                let base = (bits << (8 - n)) as usize;
                t.first[base..base + (1 << (8 - n))].fill(e);
            } else {
                let head = (bits >> (n - 8)) as usize;
                if t.first[head].len != LINK {
                    t.first[head] = Entry {
                        len: LINK,
                        a: 0,
                        b: t.more.len() as i16,
                    };
                    t.more.push([Entry::default(); 256]);
                }
                let tail = ((bits << (16 - n)) & 0xff) as usize;
                t.more[t.first[head].b as usize][tail..tail + (1 << (16 - n))].fill(e);
            }
        }
        t
    }

    /// The entry of the code at the top of the 16 bits `bits`.
    #[inline]
    pub fn get(&self, bits: u32) -> Entry {
        let e = self.first[(bits >> 8) as usize & 0xff];
        if e.len != LINK {
            return e;
        }
        match self.more.get(e.b as usize) {
            Some(m) => m[(bits & 0xff) as usize],
            None => Entry::default(),
        }
    }
}

/// The decoding tables.
pub(crate) struct Tables {
    pub increment: Vlc,
    /// By picture_coding_type minus 1.
    pub mb_type: [Vlc; 3],
    pub pattern: Vlc,
    pub motion: Vlc,
    pub dc_luma: Vlc,
    pub dc_chroma: Vlc,
    /// Table B-14 and table B-15.
    pub coeff: [Vlc; 2],
}

fn plain(t: &'static [(&'static str, u8)]) -> Vlc {
    Vlc::build(t.iter().map(|&(c, v)| (c, v, 1)))
}

fn coeff(t: Vec<(&'static str, u8, u8)>, end: &'static str) -> Vlc {
    let special = [(end, 0, END), (COEFF_ESCAPE, 0, ESCAPE)];
    Vlc::build(
        t.into_iter()
            .map(|(c, r, l)| (c, r, l as i16))
            .chain(special),
    )
}

/// The decoding tables, built at the first call.
pub(crate) fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        increment: plain(ADDRESS_INCREMENT),
        mb_type: [plain(TYPE_I), plain(TYPE_P), plain(TYPE_B)],
        pattern: plain(BLOCK_PATTERN),
        motion: plain(MOTION_CODE),
        dc_luma: plain(DC_SIZE_LUMA),
        dc_chroma: plain(DC_SIZE_CHROMA),
        coeff: [
            coeff(COEFF_ZERO.to_vec(), COEFF_END_ZERO),
            coeff(coeff_one(), COEFF_END_ONE),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefix_free(codes: &[&str]) {
        let c: Vec<(u32, u32)> = codes.iter().map(|c| code_bits(c)).collect();
        for (i, a) in c.iter().enumerate() {
            for (j, b) in c.iter().enumerate() {
                if i != j && a.1 <= b.1 {
                    assert!(
                        b.0 >> (b.1 - a.1) != a.0,
                        "{} is a prefix of {}",
                        codes[i],
                        codes[j]
                    );
                }
            }
        }
    }

    fn firsts<T>(t: &[(&'static str, T)]) -> Vec<&'static str> {
        t.iter().map(|e| e.0).collect()
    }

    #[test]
    fn the_tables_are_prefix_free_and_have_the_counts_of_the_standard() {
        assert_eq!(ADDRESS_INCREMENT.len(), 34);
        assert_eq!(BLOCK_PATTERN.len(), 64);
        assert_eq!(MOTION_CODE.len(), 17);
        assert_eq!(COEFF_ZERO.len(), 111);
        assert_eq!(coeff_one().len(), 111);
        prefix_free(&firsts(ADDRESS_INCREMENT));
        prefix_free(&firsts(TYPE_I));
        prefix_free(&firsts(TYPE_P));
        prefix_free(&firsts(TYPE_B));
        prefix_free(&firsts(BLOCK_PATTERN));
        prefix_free(&firsts(MOTION_CODE));
        prefix_free(&firsts(DC_SIZE_LUMA));
        prefix_free(&firsts(DC_SIZE_CHROMA));
        let mut zero: Vec<&str> = COEFF_ZERO.iter().map(|e| e.0).collect();
        zero.extend([COEFF_END_ZERO, COEFF_ESCAPE]);
        prefix_free(&zero);
        let mut one: Vec<&str> = coeff_one().iter().map(|e| e.0).collect();
        one.extend([COEFF_END_ONE, COEFF_ESCAPE]);
        prefix_free(&one);
    }

    #[test]
    fn every_value_occurs_once() {
        let mut p: Vec<u8> = BLOCK_PATTERN.iter().map(|e| e.1).collect();
        p.sort_unstable();
        assert_eq!(p, (0..64).collect::<Vec<u8>>());
        for t in [COEFF_ZERO.to_vec(), coeff_one()] {
            let mut pairs: Vec<(u8, u8)> = t.iter().map(|e| (e.1, e.2)).collect();
            pairs.sort_unstable();
            let n = pairs.len();
            pairs.dedup();
            assert_eq!(pairs.len(), n);
        }
        let mut a: Vec<(u8, u8)> = COEFF_ZERO.iter().map(|e| (e.1, e.2)).collect();
        let mut b: Vec<(u8, u8)> = coeff_one().iter().map(|e| (e.1, e.2)).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
        for scan in [ZIGZAG, ALTERNATE] {
            let mut s = scan.to_vec();
            s.sort_unstable();
            assert_eq!(s, (0..64).collect::<Vec<u8>>());
        }
    }

    #[test]
    fn the_decoding_tables_find_every_code() {
        let t = tables();
        for &(code, run, level) in COEFF_ZERO {
            let (bits, n) = code_bits(code);
            let e = t.coeff[0].get(bits << (16 - n));
            assert_eq!((e.len as u32, e.a, e.b), (n, run, level as i16), "{code}");
        }
        for (code, run, level) in coeff_one() {
            let (bits, n) = code_bits(code);
            let e = t.coeff[1].get((bits << (16 - n)) | ((1 << (16 - n)) - 1));
            assert_eq!((e.len as u32, e.a, e.b), (n, run, level as i16), "{code}");
        }
        for &(code, v) in ADDRESS_INCREMENT {
            let (bits, n) = code_bits(code);
            let e = t.increment.get(bits << (16 - n));
            assert_eq!((e.len as u32, e.a), (n, v));
        }
        assert_eq!(t.coeff[0].get(0).len, 0);
        assert_eq!(t.increment.get(0).len, 0);
    }
}
