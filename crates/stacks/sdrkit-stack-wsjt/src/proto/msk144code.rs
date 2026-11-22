//! The (128,90) LDPC code of MSK144: 90 information bits (the 77 message bits and their CRC-13), then 38 parity bits. Every
//! code word bit is in 3 checks, and the code word is in transmission order with no column reordering. The code is held in
//! two forms of its check matrix and as the parity part of its generator. [`super::codes::msk144`] builds the decoder's
//! code from the check lists.
//!
//! The (32,16) code of the short messages `<CALL1 CALL2> RPT` is here too, with its encoder: a 16-bit value 16 · h + r
//! (h the 12-bit hash of the two calls, r the report index) becomes a 32-bit code word, sent after an 8-bit sync word as
//! a 40-bit frame. The stack does not send or receive short messages.

/// The 38 checks of the (128,90) code: entry i lists the code word bits of check i, in increasing order.
#[rustfmt::skip]
pub static MSK144_CHECKS: [&[u8]; 38] = [
    /*   0 */ &[1, 14, 26, 39, 52, 64, 76, 90, 93, 114],
    /*   1 */ &[2, 5, 27, 40, 53, 65, 77, 91, 93, 119],
    /*   2 */ &[3, 15, 28, 41, 54, 66, 76, 89, 92, 105, 117],
    /*   3 */ &[4, 16, 29, 42, 51, 63, 78, 91, 101, 118],
    /*   4 */ &[5, 17, 30, 43, 55, 67, 79, 88, 94, 107, 124],
    /*   5 */ &[6, 13, 31, 44, 56, 67, 78, 89, 95, 115, 120],
    /*   6 */ &[3, 18, 32, 42, 57, 68, 80, 96, 106, 120],
    /*   7 */ &[1, 19, 29, 38, 53, 69, 79, 97, 106, 127],
    /*   8 */ &[2, 20, 33, 45, 58, 66, 78, 98, 106, 122],
    /*   9 */ &[7, 14, 28, 46, 55, 70, 81, 99, 110, 127],
    /*  10 */ &[8, 21, 33, 43, 51, 71, 82, 100, 102, 125],
    /*  11 */ &[9, 16, 25, 47, 59, 72, 83, 90, 109, 120],
    /*  12 */ &[6, 22, 34, 37, 54, 72, 81, 100, 108, 119],
    /*  13 */ &[10, 18, 35, 48, 52, 69, 84, 101, 103, 125],
    /*  14 */ &[9, 19, 36, 45, 57, 70, 84, 104, 108, 121],
    /*  15 */ &[4, 22, 36, 46, 56, 73, 85, 92, 109, 124],
    /*  16 */ &[11, 12, 26, 40, 60, 67, 86, 96, 108, 112],
    /*  17 */ &[10, 15, 37, 44, 57, 71, 77, 98, 107, 114],
    /*  18 */ &[3, 21, 30, 40, 59, 73, 81, 104, 111, 114],
    /*  19 */ &[11, 23, 31, 38, 61, 62, 87, 98, 101, 117],
    /*  20 */ &[0, 18, 24, 44, 61, 74, 76, 99, 111, 118],
    /*  21 */ &[5, 24, 32, 49, 58, 70, 82, 97, 116, 117],
    /*  22 */ &[9, 15, 38, 50, 52, 65, 82, 94, 110, 123],
    /*  23 */ &[8, 23, 34, 41, 58, 75, 88, 93, 113, 121],
    /*  24 */ &[6, 16, 36, 49, 53, 74, 87, 110, 113, 122],
    /*  25 */ &[10, 24, 34, 47, 60, 64, 87, 104, 115, 124],
    /*  26 */ &[8, 20, 31, 48, 54, 68, 83, 85, 94, 112, 118],
    /*  27 */ &[1, 17, 27, 46, 62, 72, 86, 95, 105, 125],
    /*  28 */ &[11, 25, 30, 48, 63, 71, 80, 99, 105, 119],
    /*  29 */ &[12, 14, 27, 47, 50, 75, 84, 92, 102, 122],
    /*  30 */ &[7, 19, 35, 43, 56, 74, 77, 90, 112, 116],
    /*  31 */ &[4, 20, 28, 39, 50, 69, 80, 95, 116, 121],
    /*  32 */ &[7, 25, 33, 39, 61, 73, 79, 91, 115, 126],
    /*  33 */ &[0, 12, 13, 35, 41, 63, 65, 83, 100, 107],
    /*  34 */ &[13, 21, 26, 49, 62, 68, 88, 103, 109, 127],
    /*  35 */ &[0, 17, 29, 45, 59, 64, 89, 96, 113, 126],
    /*  36 */ &[2, 22, 32, 51, 55, 75, 86, 103, 111, 123],
    /*  37 */ &[23, 37, 42, 60, 66, 85, 97, 102, 123, 126],
];

/// The checks of each of the 128 code word bits of the (128,90) code: the matrix of [`MSK144_CHECKS`] read by columns.
#[rustfmt::skip]
pub static MSK144_VARIABLES: [[u8; 3]; 128] = [
    /*   0 */ [20, 33, 35],
    /*   1 */ [0, 7, 27],
    /*   2 */ [1, 8, 36],
    /*   3 */ [2, 6, 18],
    /*   4 */ [3, 15, 31],
    /*   5 */ [1, 4, 21],
    /*   6 */ [5, 12, 24],
    /*   7 */ [9, 30, 32],
    /*   8 */ [10, 23, 26],
    /*   9 */ [11, 14, 22],
    /*  10 */ [13, 17, 25],
    /*  11 */ [16, 19, 28],
    /*  12 */ [16, 29, 33],
    /*  13 */ [5, 33, 34],
    /*  14 */ [0, 9, 29],
    /*  15 */ [2, 17, 22],
    /*  16 */ [3, 11, 24],
    /*  17 */ [4, 27, 35],
    /*  18 */ [6, 13, 20],
    /*  19 */ [7, 14, 30],
    /*  20 */ [8, 26, 31],
    /*  21 */ [10, 18, 34],
    /*  22 */ [12, 15, 36],
    /*  23 */ [19, 23, 37],
    /*  24 */ [20, 21, 25],
    /*  25 */ [11, 28, 32],
    /*  26 */ [0, 16, 34],
    /*  27 */ [1, 27, 29],
    /*  28 */ [2, 9, 31],
    /*  29 */ [3, 7, 35],
    /*  30 */ [4, 18, 28],
    /*  31 */ [5, 19, 26],
    /*  32 */ [6, 21, 36],
    /*  33 */ [8, 10, 32],
    /*  34 */ [12, 23, 25],
    /*  35 */ [13, 30, 33],
    /*  36 */ [14, 15, 24],
    /*  37 */ [12, 17, 37],
    /*  38 */ [7, 19, 22],
    /*  39 */ [0, 31, 32],
    /*  40 */ [1, 16, 18],
    /*  41 */ [2, 23, 33],
    /*  42 */ [3, 6, 37],
    /*  43 */ [4, 10, 30],
    /*  44 */ [5, 17, 20],
    /*  45 */ [8, 14, 35],
    /*  46 */ [9, 15, 27],
    /*  47 */ [11, 25, 29],
    /*  48 */ [13, 26, 28],
    /*  49 */ [21, 24, 34],
    /*  50 */ [22, 29, 31],
    /*  51 */ [3, 10, 36],
    /*  52 */ [0, 13, 22],
    /*  53 */ [1, 7, 24],
    /*  54 */ [2, 12, 26],
    /*  55 */ [4, 9, 36],
    /*  56 */ [5, 15, 30],
    /*  57 */ [6, 14, 17],
    /*  58 */ [8, 21, 23],
    /*  59 */ [11, 18, 35],
    /*  60 */ [16, 25, 37],
    /*  61 */ [19, 20, 32],
    /*  62 */ [19, 27, 34],
    /*  63 */ [3, 28, 33],
    /*  64 */ [0, 25, 35],
    /*  65 */ [1, 22, 33],
    /*  66 */ [2, 8, 37],
    /*  67 */ [4, 5, 16],
    /*  68 */ [6, 26, 34],
    /*  69 */ [7, 13, 31],
    /*  70 */ [9, 14, 21],
    /*  71 */ [10, 17, 28],
    /*  72 */ [11, 12, 27],
    /*  73 */ [15, 18, 32],
    /*  74 */ [20, 24, 30],
    /*  75 */ [23, 29, 36],
    /*  76 */ [0, 2, 20],
    /*  77 */ [1, 17, 30],
    /*  78 */ [3, 5, 8],
    /*  79 */ [4, 7, 32],
    /*  80 */ [6, 28, 31],
    /*  81 */ [9, 12, 18],
    /*  82 */ [10, 21, 22],
    /*  83 */ [11, 26, 33],
    /*  84 */ [13, 14, 29],
    /*  85 */ [15, 26, 37],
    /*  86 */ [16, 27, 36],
    /*  87 */ [19, 24, 25],
    /*  88 */ [4, 23, 34],
    /*  89 */ [2, 5, 35],
    /*  90 */ [0, 11, 30],
    /*  91 */ [1, 3, 32],
    /*  92 */ [2, 15, 29],
    /*  93 */ [0, 1, 23],
    /*  94 */ [4, 22, 26],
    /*  95 */ [5, 27, 31],
    /*  96 */ [6, 16, 35],
    /*  97 */ [7, 21, 37],
    /*  98 */ [8, 17, 19],
    /*  99 */ [9, 20, 28],
    /* 100 */ [10, 12, 33],
    /* 101 */ [3, 13, 19],
    /* 102 */ [10, 29, 37],
    /* 103 */ [13, 34, 36],
    /* 104 */ [14, 18, 25],
    /* 105 */ [2, 27, 28],
    /* 106 */ [6, 7, 8],
    /* 107 */ [4, 17, 33],
    /* 108 */ [12, 14, 16],
    /* 109 */ [11, 15, 34],
    /* 110 */ [9, 22, 24],
    /* 111 */ [18, 20, 36],
    /* 112 */ [16, 26, 30],
    /* 113 */ [23, 24, 35],
    /* 114 */ [0, 17, 18],
    /* 115 */ [5, 25, 32],
    /* 116 */ [21, 30, 31],
    /* 117 */ [2, 19, 21],
    /* 118 */ [3, 20, 26],
    /* 119 */ [1, 12, 28],
    /* 120 */ [5, 6, 11],
    /* 121 */ [14, 23, 31],
    /* 122 */ [8, 24, 29],
    /* 123 */ [22, 36, 37],
    /* 124 */ [4, 15, 25],
    /* 125 */ [10, 13, 27],
    /* 126 */ [32, 35, 37],
    /* 127 */ [7, 9, 34],
];

/// The parity part of the generator of the (128,90) code: row j, as hex digits (most significant bit first, two zero pad bits at the end), gives the 38 parity bits of the information word with only bit j set.
#[rustfmt::skip]
pub static MSK144_PARITY: [&str; 90] = [
    /*   0 */ "be2b060760",
    /*   1 */ "6bcea09c7c",
    /*   2 */ "a2c0b5b584",
    /*   3 */ "54d784c104",
    /*   4 */ "401bbe7ed4",
    /*   5 */ "3b821af38c",
    /*   6 */ "060ca901d4",
    /*   7 */ "57c382d908",
    /*   8 */ "dde1eb1b08",
    /*   9 */ "5c7de7b28c",
    /*  10 */ "4760522e3c",
    /*  11 */ "4ea6cd4d14",
    /*  12 */ "96d8f24d3c",
    /*  13 */ "ac1854b850",
    /*  14 */ "fad907c2f0",
    /*  15 */ "7e5587f0a4",
    /*  16 */ "dfb2e56850",
    /*  17 */ "15d6bbae60",
    /*  18 */ "c439dcf4e4",
    /*  19 */ "4919ba56b4",
    /*  20 */ "9ef04b0e10",
    /*  21 */ "40f4d44624",
    /*  22 */ "38999dce3c",
    /*  23 */ "1c7249cd18",
    /*  24 */ "57c18ff840",
    /*  25 */ "2ce67e7090",
    /*  26 */ "31b34e2df4",
    /*  27 */ "08f0480c90",
    /*  28 */ "9e58b87dbc",
    /*  29 */ "1c6640cd5c",
    /*  30 */ "323e56ff08",
    /*  31 */ "1ecc3cc7b8",
    /*  32 */ "2ffdf90b00",
    /*  33 */ "c962c9ef08",
    /*  34 */ "877fdad9e8",
    /*  35 */ "87ed259384",
    /*  36 */ "8c2673c158",
    /*  37 */ "0cc3b63264",
    /*  38 */ "152bd897a4",
    /*  39 */ "973dd7dca0",
    /*  40 */ "43598c10d8",
    /*  41 */ "40c0f27f24",
    /*  42 */ "3e70d7b6d4",
    /*  43 */ "0351125a3c",
    /*  44 */ "7114959288",
    /*  45 */ "b4e6c536b4",
    /*  46 */ "650f266c14",
    /*  47 */ "9842b746ec",
    /*  48 */ "10c92eb5d0",
    /*  49 */ "1168ddc6e8",
    /*  50 */ "25fd6369ec",
    /*  51 */ "2ba27c5a8c",
    /*  52 */ "c1fa5525ec",
    /*  53 */ "08fc400cc4",
    /*  54 */ "9d0119642c",
    /*  55 */ "327e52ff08",
    /*  56 */ "75d505a08c",
    /*  57 */ "93335f1c74",
    /*  58 */ "26a4e851fc",
    /*  59 */ "f94f00297c",
    /*  60 */ "d8e5e07b54",
    /*  61 */ "e6dab0f41c",
    /*  62 */ "af73f8e898",
    /*  63 */ "36a417be90",
    /*  64 */ "e00f360c18",
    /*  65 */ "11bc2ea984",
    /*  66 */ "3ad6a18ce4",
    /*  67 */ "a28201b3a0",
    /*  68 */ "04eb7e32e0",
    /*  69 */ "8a7760fa5c",
    /*  70 */ "0359833990",
    /*  71 */ "6b8aa41c68",
    /*  72 */ "d3e9535aa4",
    /*  73 */ "ec9533c5e8",
    /*  74 */ "74696ec960",
    /*  75 */ "b30db12590",
    /*  76 */ "ff26f1e8d0",
    /*  77 */ "68820539e8",
    /*  78 */ "da009b4024",
    /*  79 */ "2b4f1f6310",
    /*  80 */ "daa7ed7a14",
    /*  81 */ "1d666fe41c",
    /*  82 */ "2a478721b4",
    /*  83 */ "a8f32a88b0",
    /*  84 */ "f949886bb0",
    /*  85 */ "30c02eb5c0",
    /*  86 */ "0ee7e6705c",
    /*  87 */ "a92a7a9004",
    /*  88 */ "3b27c3a5f0",
    /*  89 */ "ad9884ea78",
];


/// The sync word of the 40-bit short-message frame.
pub const SHORT_SYNC: [u8; 8] = [1, 0, 1, 1, 0, 0, 0, 1];

/// The reports of a short message, by report index.
pub const SHORT_REPORTS: [&str; 16] = [
    "-03", "+00", "+03", "+06", "+10", "+13", "+16", "R-03", "R+00", "R+03", "R+06", "R+10", "R+13", "R+16", "RRR", "73",
];

/// The sent position of each bit of the encoder's word: entry t is the code word bit that word bit t becomes. Word bits
/// 0 … 15 are the parity bits, word bits 16 … 31 the message bits 0 … 15.
#[rustfmt::skip]
pub static SHORT_ORDER: [u8; 32] = [
     4,  1,  2,  3,  0,  8,  6, 10, 13, 28, 20, 23, 17, 15, 27, 25,
    16, 12, 18, 19,  7, 21, 22, 11, 24,  5, 26, 14,  9, 29, 30, 31,
];

/// The 16 checks of the (32,16) code, on the code word in sent order: entry i lists the code word bits of check i.
#[rustfmt::skip]
pub static SHORT_CHECKS: [&[u8]; 16] = [
    /*  0 */ &[0, 6, 11, 17, 20, 26, 30],
    /*  1 */ &[1, 7, 12, 17, 23, 27],
    /*  2 */ &[1, 8, 13, 18, 22, 28],
    /*  3 */ &[2, 7, 11, 19, 22, 29],
    /*  4 */ &[3, 7, 14, 18, 21, 30],
    /*  5 */ &[0, 5, 12, 18, 24, 29],
    /*  6 */ &[4, 8, 15, 20, 24, 25],
    /*  7 */ &[2, 9, 10, 17, 25, 31],
    /*  8 */ &[5, 8, 14, 19, 26, 31],
    /*  9 */ &[4, 9, 12, 21, 26],
    /* 10 */ &[3, 6, 13, 19, 25, 27],
    /* 11 */ &[3, 9, 11, 16, 24, 28],
    /* 12 */ &[0, 10, 15, 21, 27, 28],
    /* 13 */ &[1, 10, 14, 16, 20, 29],
    /* 14 */ &[2, 5, 13, 15, 23, 30],
    /* 15 */ &[4, 6, 16, 22, 23, 31],
];

/// The checks of each of the 32 code word bits of the (32,16) code, in sent order: [`SHORT_CHECKS`] read by columns.
#[rustfmt::skip]
pub static SHORT_VARIABLES: [[u8; 3]; 32] = [
    /*  0 */ [0, 5, 12],
    /*  1 */ [1, 2, 13],
    /*  2 */ [3, 7, 14],
    /*  3 */ [4, 10, 11],
    /*  4 */ [6, 9, 15],
    /*  5 */ [5, 8, 14],
    /*  6 */ [0, 10, 15],
    /*  7 */ [1, 3, 4],
    /*  8 */ [2, 6, 8],
    /*  9 */ [7, 9, 11],
    /* 10 */ [7, 12, 13],
    /* 11 */ [0, 3, 11],
    /* 12 */ [1, 5, 9],
    /* 13 */ [2, 10, 14],
    /* 14 */ [4, 8, 13],
    /* 15 */ [6, 12, 14],
    /* 16 */ [11, 13, 15],
    /* 17 */ [0, 1, 7],
    /* 18 */ [2, 4, 5],
    /* 19 */ [3, 8, 10],
    /* 20 */ [0, 6, 13],
    /* 21 */ [4, 9, 12],
    /* 22 */ [2, 3, 15],
    /* 23 */ [1, 14, 15],
    /* 24 */ [5, 6, 11],
    /* 25 */ [6, 7, 10],
    /* 26 */ [0, 8, 9],
    /* 27 */ [1, 10, 12],
    /* 28 */ [2, 11, 12],
    /* 29 */ [3, 5, 13],
    /* 30 */ [0, 4, 14],
    /* 31 */ [7, 8, 15],
];

/// The parity part of the generator of the (32,16) code: row j, as hex digits (most significant bit first), gives the 16
/// parity bits p0 … p15 of the message with only bit j set.
#[rustfmt::skip]
pub static SHORT_PARITY: [&str; 16] = [
    /*  0 */ "02e4",
    /*  1 */ "c8b2",
    /*  2 */ "1b7c",
    /*  3 */ "671b",
    /*  4 */ "7273",
    /*  5 */ "93e5",
    /*  6 */ "630d",
    /*  7 */ "20ca",
    /*  8 */ "09e5",
    /*  9 */ "4f11",
    /* 10 */ "c629",
    /* 11 */ "1541",
    /* 12 */ "c349",
    /* 13 */ "290f",
    /* 14 */ "527b",
    /* 15 */ "4514",
];

/// The 12-bit hash of the two calls of a short message (`CALL1 CALL2`): the text upper-cased, runs of blanks made one blank,
/// padded with blanks to 37 characters, hashed with the 15-bit callsign hash function and cut to its low 12 bits.
pub fn short_hash(calls: &str) -> u16 {
    let mut t = calls.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_uppercase().into_bytes();
    t.resize(37.max(t.len()), b' ');
    (super::wsprmsg::lookup3(&t, 146) & 0x7FFF & 0xFFF) as u16
}

/// The 16-bit value 16 · h + r of a short message `<CALL1 CALL2> RPT`, or `None` when the text has another form or a report
/// that is not one of [`SHORT_REPORTS`].
pub fn short_value(text: &str) -> Option<u16> {
    let t = text.trim();
    let (calls, rest) = t.strip_prefix('<')?.split_once('>')?;
    if calls.split_whitespace().count() != 2 {
        return None;
    }
    let r = SHORT_REPORTS.iter().position(|&x| x == rest.trim())?;
    Some(16 * short_hash(calls) + r as u16)
}

/// The 32 code word bits of the short-message value `v`, in sent order. Message bit j is bit j of `v` (least significant
/// bit first); parity bit i is the exclusive-or of the message bits j whose generator row has bit i set.
pub fn short_codeword(v: u16) -> [u8; 32] {
    let mut w = [0u8; 32];
    for j in 0..16 {
        let m = (v >> j & 1) as u8;
        w[16 + j] = m;
        if m == 1 {
            for (i, p) in w[..16].iter_mut().enumerate() {
                *p ^= parity_bit(SHORT_PARITY[j], i);
            }
        }
    }
    let mut c = [0u8; 32];
    for (t, &b) in w.iter().enumerate() {
        c[SHORT_ORDER[t] as usize] = b;
    }
    c
}

/// The 40-bit frame of the short-message value `v`: [`SHORT_SYNC`], then the code word of [`short_codeword`].
pub fn short_frame(v: u16) -> [u8; 40] {
    let mut f = [0u8; 40];
    f[..8].copy_from_slice(&SHORT_SYNC);
    f[8..].copy_from_slice(&short_codeword(v));
    f
}

/// Bit `i` of a hex generator row, most significant bit of the first digit first.
fn parity_bit(row: &str, i: usize) -> u8 {
    let d = row.as_bytes()[i / 4];
    let d = (d as char).to_digit(16).unwrap_or(0) as u8;
    d >> (3 - i % 4) & 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::codes::{self, tests::*};

    #[test]
    fn the_msk144_tables_agree_and_have_their_weights() {
        check_table(codes::msk144(), &MSK144_CHECKS, &MSK144_VARIABLES, &MSK144_PARITY);
        assert_eq!(row_weights(&MSK144_CHECKS), [(10, 34), (11, 4)]);
    }

    /// Whether `c` (sent order) satisfies every check of the (32,16) code.
    fn short_checks_hold(c: &[u8; 32]) -> bool {
        SHORT_CHECKS.iter().all(|k| k.iter().fold(0, |a, &b| a ^ c[b as usize]) == 0)
    }

    fn bits(s: &str) -> Vec<u8> {
        s.bytes().map(|b| b - b'0').collect()
    }

    #[test]
    fn the_short_message_tables_agree() {
        let mut cols = vec![Vec::new(); 32];
        for (i, k) in SHORT_CHECKS.iter().enumerate() {
            for &b in *k {
                cols[b as usize].push(i as u8);
            }
        }
        for (j, v) in SHORT_VARIABLES.iter().enumerate() {
            assert_eq!(cols[j], v.to_vec(), "bit {j}");
        }
        assert_eq!(row_weights(&SHORT_CHECKS), [(5, 1), (6, 14), (7, 1)]);
        let mut order = SHORT_ORDER.to_vec();
        order.sort_unstable();
        assert_eq!(order, (0..32).collect::<Vec<u8>>());
    }

    #[test]
    fn every_short_code_word_satisfies_every_check() {
        for j in 0..16 {
            assert!(short_checks_hold(&short_codeword(1 << j)), "message bit {j}");
        }
        assert_eq!(short_codeword(0), [0; 32]);
        assert_eq!(short_codeword(1).to_vec(), bits("00000010000001011000100000001000"));
        // Minimum distance 4, with 3 code words of weight 4.
        let weights: Vec<u32> = (1..=u16::MAX)
            .map(|v| short_codeword(v).iter().map(|&b| u32::from(b)).sum())
            .collect();
        assert_eq!(weights.iter().min(), Some(&4));
        assert_eq!(weights.iter().filter(|&&w| w == 4).count(), 3);
    }

    #[test]
    fn the_printed_short_message() {
        assert_eq!(short_hash("K1ABC W9XYZ"), 0xFA2);
        assert_eq!(short_hash("k1abc   w9xyz"), 0xFA2);
        let v = short_value("<K1ABC W9XYZ> RRR").unwrap();
        assert_eq!(v, 0xFA2E);
        let m: Vec<u8> = (0..16).map(|j| (v >> j & 1) as u8).collect();
        assert_eq!(m, bits("0111010001011111"));
        let c = short_codeword(v);
        assert_eq!(c.to_vec(), bits("00001100010010100111010001001111"));
        assert!(short_checks_hold(&c));
        let f = short_frame(v);
        assert_eq!(f.to_vec(), bits("1011000100001100010010100111010001001111"));
        assert_eq!(crate::encode::msk_tones(&f), bits("1000011001000001100010111100100110000101"));
        for t in ["<K1ABC W9XYZ> -05", "K1ABC W9XYZ RRR", "<K1ABC> RRR"] {
            assert_eq!(short_value(t), None, "{t}");
        }
    }

    #[test]
    fn the_printed_msk144_code_words() {
        let c = codes::msk144();
        check_word(c, "80000000000000000000002f8ac181d8", 18);
        check_word(c, "000000204def1a8a1988f065a100982a", 40);
    }
}
