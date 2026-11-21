//! Named tables of the digital voice stacks: the NXDN puncturing patterns, the dPMR interleaves, the NXDN vocoder test frames,
//! and a vocoder frame layout loaded at run time for frames whose bit order no free standard prints.
//! Standard: NXDN TS 1-A Version 1.3 clauses 4.5 and 7.2, ETSI TS 102 490 V1.9.1 clause 7

use crate::conv::Puncture;
use crate::error::{FecError, Param, Result};
use crate::small::SmallCode;
use sdrkit_util_bits::interleave::{Permutation, ReadOrder};

// ----- NXDN puncturing -----
//
// NXDN TS 1-A prints each puncturing matrix as two rows, G1 over G2. The encoder sends G1 and G2 in turn, so the serial
// pattern takes one column at a time: G1 then G2. The short CAC (inbound) has no puncturing (clause 4.5.1.3 (5) says "None").

/// NXDN CAC (outbound) puncturing: 12 of every 14 code bits are sent, so 350 code bits give 300. G1 row `1111111`, G2 row
/// `1011101`. Standard: NXDN TS 1-A Version 1.3 clause 4.5.1.1 (5).
pub const NXDN_CAC_PUNCTURE: Puncture<'static> =
    Puncture::new(&[1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1]);

/// NXDN long CAC (inbound) puncturing: 21 of every 26 code bits are sent, so 312 code bits give 252. G1 row `1111111111111`,
/// G2 row `0110001110111`. Standard: NXDN TS 1-A Version 1.3 clause 4.5.1.2 (5).
pub const NXDN_LONG_CAC_PUNCTURE: Puncture<'static> = Puncture::new(&[
    1, 0, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 1,
]);

/// NXDN SACCH puncturing: 10 of every 12 code bits are sent, so 72 code bits give 60. G1 row `111111`, G2 row `110110`.
/// Standard: NXDN TS 1-A Version 1.3 clause 4.5.2.1 (5).
pub const NXDN_SACCH_PUNCTURE: Puncture<'static> =
    Puncture::new(&[1, 1, 1, 1, 1, 0, 1, 1, 1, 1, 1, 0]);

/// NXDN FACCH1 puncturing: 3 of every 4 code bits are sent, so 192 code bits give 144. G1 row `11`, G2 row `01`.
/// Standard: NXDN TS 1-A Version 1.3 clause 4.5.2.2 (5).
pub const NXDN_FACCH1_PUNCTURE: Puncture<'static> = Puncture::new(&[1, 0, 1, 1]);

/// NXDN UDCH puncturing: 12 of every 14 code bits are sent, so 406 code bits give 348. G1 row `1111111`, G2 row `1011101`.
/// Standard: NXDN TS 1-A Version 1.3 clause 4.5.2.3 (5).
pub const NXDN_UDCH_PUNCTURE: Puncture<'static> =
    Puncture::new(&[1, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 0, 1, 1]);

/// NXDN FACCH2 puncturing: FACCH2 uses the UDCH coding. Standard: NXDN TS 1-A Version 1.3 clause 4.5.2.3 (5).
pub const NXDN_FACCH2_PUNCTURE: Puncture<'static> = NXDN_UDCH_PUNCTURE;

// ----- dPMR interleaves -----

/// The dPMR TCH interleave of 72 bits: six Hamming (12,8) words go into the columns of a 12 x 6 matrix and leave by rows.
/// [`Permutation::apply`] interleaves (input 1, 2, 3 ... produces 1, 13, 25, 37, 49, 61, 2, 14 ... counted from 1).
/// [`Permutation::invert`] de-interleaves. The control channel (CCH) and each 72-bit part of type 2 data use it.
/// Standard: ETSI TS 102 490 V1.9.1 clause 7.5 table 7.2 (TS 102 658 V2.6.1 clause 7.4 table 7.3).
pub fn dpmr_tch_interleave() -> Permutation {
    dpmr_matrix(6)
}

/// The dPMR header (HI) interleave of 120 bits: ten Hamming (12,8) words go into the columns of a 12 x 10 matrix and leave by
/// rows. [`Permutation::apply`] interleaves (input 1, 2, 3 ... produces 1, 13, 25 ... 109, 2, 14 ... counted from 1).
/// Standard: ETSI TS 102 490 V1.9.1 clause 7.5 table 7.3 (TS 102 658 V2.6.1 clause 7.4 table 7.4, MI and HI fields).
pub fn dpmr_hi_interleave() -> Permutation {
    dpmr_matrix(10)
}

/// Words of 12 bits written as the columns of a 12-row matrix and read by rows: input `i` goes to `(i mod 12) * words + i / 12`.
fn dpmr_matrix(words: usize) -> Permutation {
    // Written row by row into `words` rows of 12 and read by columns is the same map.
    Permutation::row_column(words, 12, ReadOrder::Columns).expect("a whole matrix")
}

// ----- Vocoder frames -----

/// A vocoder test frame: the vocoder's information bits and the channel bits after its FEC, each packed with the first bit
/// sent in the most significant place of the first byte (unused low bits of the last byte are 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VocoderTestFrame {
    /// Number of information bits.
    pub info_bits: usize,
    /// The information bits.
    pub info: &'static [u8],
    /// Number of channel bits.
    pub coded_bits: usize,
    /// The channel bits with the vocoder's FEC, in the order sent.
    pub coded: &'static [u8],
}

impl VocoderTestFrame {
    /// Information bit `i` (0 is the first), or 0 past the end.
    pub fn info_bit(&self, i: usize) -> u8 {
        packed_bit(self.info, i, self.info_bits)
    }

    /// Channel bit `i` (0 is the first sent), or 0 past the end.
    pub fn coded_bit(&self, i: usize) -> u8 {
        packed_bit(self.coded, i, self.coded_bits)
    }
}

fn packed_bit(bytes: &[u8], i: usize, len: usize) -> u8 {
    if i >= len {
        return 0;
    }
    bytes.get(i / 8).map_or(0, |b| (b >> (7 - i % 8)) & 1)
}

/// The NXDN EHR (AMBE+2 half rate, 3600 bit/s) tone test frame: a 1031 Hz tone at -22 dBm0, 49 information bits and 72
/// channel bits. Standard: NXDN TS 1-A Version 1.3 clause 7.2.1.1.
pub const NXDN_EHR_TONE: VocoderTestFrame = VocoderTestFrame {
    info_bits: 49,
    info: &[0xFE, 0xE2, 0x12, 0x12, 0x12, 0x10, 0x00],
    coded_bits: 72,
    coded: &[0xCE, 0xA8, 0xFE, 0x83, 0xAC, 0xC4, 0x58, 0x20, 0x0A],
};

/// The NXDN EHR (AMBE+2 half rate) silence test frame: 49 information bits and 72 channel bits.
/// Standard: NXDN TS 1-A Version 1.3 clause 7.2.1.2.
pub const NXDN_EHR_SILENCE: VocoderTestFrame = VocoderTestFrame {
    info_bits: 49,
    info: &[0xF8, 0x01, 0xA9, 0x9F, 0x8C, 0xE0, 0x80],
    coded_bits: 72,
    coded: &[0xB9, 0xE8, 0x81, 0x52, 0x61, 0x73, 0x00, 0x2A, 0x6B],
};

/// The NXDN EFR (AMBE+2 full rate, 7200 bit/s) tone test frames #1 and #2, sent in turn: a 1011 Hz tone at -22 dBm0, 88
/// information bits and 144 channel bits each. Standard: NXDN TS 1-A Version 1.3 clause 7.2.2.1.
pub const NXDN_EFR_TONE: [VocoderTestFrame; 2] = [
    VocoderTestFrame {
        info_bits: 88,
        info: &[
            0x09, 0xB0, 0x88, 0x0C, 0xC6, 0x21, 0xF6, 0x80, 0xA8, 0x26, 0x00,
        ],
        coded_bits: 144,
        coded: &[
            0x38, 0x92, 0x84, 0x90, 0xD4, 0x33, 0xC0, 0xBE, 0x1B, 0x91, 0x84, 0x4F, 0xF0, 0x58,
            0xA5, 0x89, 0xD8, 0x39,
        ],
    },
    VocoderTestFrame {
        info_bits: 88,
        info: &[
            0x09, 0xB0, 0x88, 0x0C, 0xC6, 0x21, 0xF6, 0x80, 0xA8, 0x26, 0x00,
        ],
        coded_bits: 144,
        coded: &[
            0x38, 0x92, 0x84, 0x90, 0xD4, 0x33, 0xC0, 0xBE, 0x1B, 0x91, 0x84, 0x4F, 0xF0, 0x58,
            0xA5, 0x89, 0xD8, 0x3B,
        ],
    },
];

/// The NXDN EFR (AMBE+2 full rate) silence test frames #1 and #2, sent in turn: 88 information bits and 144 channel bits
/// each. Standard: NXDN TS 1-A Version 1.3 clause 7.2.2.2.
pub const NXDN_EFR_SILENCE: [VocoderTestFrame; 2] = [
    VocoderTestFrame {
        info_bits: 88,
        info: &[
            0x04, 0x0C, 0xFD, 0x7B, 0xFB, 0x7D, 0xF2, 0x7B, 0x3D, 0x9E, 0x44,
        ],
        coded_bits: 144,
        coded: &[
            0x6C, 0x42, 0xE8, 0x5D, 0xE2, 0xE8, 0x26, 0x93, 0x63, 0xD9, 0x81, 0xF9, 0xBE, 0x23,
            0xB1, 0x8A, 0xE0, 0x04,
        ],
    },
    VocoderTestFrame {
        info_bits: 88,
        info: &[
            0x04, 0x0C, 0xFD, 0x7B, 0xFB, 0x7D, 0xF2, 0x7B, 0x3D, 0x9E, 0x45,
        ],
        coded_bits: 144,
        coded: &[
            0x6C, 0x42, 0xE8, 0x5D, 0xE2, 0xE8, 0x26, 0x93, 0x63, 0xD9, 0x81, 0xF9, 0xBE, 0x23,
            0xB1, 0x8A, 0xE0, 0x06,
        ],
    },
];

/// The code word lengths of an AMBE+2 half-rate frame: C0 is a Golay (24,12) word, C1 a Golay (23,12) word, C2 and C3 are
/// sent without FEC. The 49 information bits are the 12 of C0, the 12 of C1, then C2 and C3 (49 + 23 = 72 bits, NXDN TS 1-A
/// clause 7.1).
pub const AMBE_HALF_RATE_WORDS: [usize; 4] = [24, 23, 11, 14];

/// The most code words in a [`VoiceLayout`].
pub const MAX_VOICE_WORDS: usize = 16;

/// The bit order of a vocoder frame on the channel: where each channel bit goes in the code words of the vocoder's FEC. The
/// table comes from outside at run time, because no free standard prints the AMBE, AMBE+2 or IMBE orders.
///
/// The code words are held one after another, each with its first bit (the most significant) first.
///
/// ```
/// use sdrkit_algo_fec::voice::VoiceLayout;
/// // Two words of 2 bits sent in turn: channel a0 b0 a1 b1.
/// let layout = VoiceLayout::new(&[0, 2, 1, 3], &[2, 2]).unwrap();
/// let mut words = [0u8; 4];
/// layout.split(&[1u8, 0, 1, 1], &mut words).unwrap();
/// assert_eq!(words, [1, 1, 0, 1]);
/// assert_eq!(layout.word(&words, 1).unwrap(), 0b01);
/// let mut channel = [0u8; 4];
/// layout.join(&words, &mut channel).unwrap();
/// assert_eq!(channel, [1, 0, 1, 1]);
/// ```
#[derive(Clone, Debug)]
pub struct VoiceLayout {
    order: Permutation,
    lens: [usize; MAX_VOICE_WORDS],
    count: usize,
}

impl VoiceLayout {
    /// The layout where channel bit `i` goes to place `dest[i]` of the code words, and the words have the lengths `word_lens`
    /// (first word first). An error when `dest` is not a permutation, when the lengths do not add up to `dest.len()`, when a
    /// length is 0 or above 32, or when there are more than [`MAX_VOICE_WORDS`] words.
    pub fn new(dest: &[u16], word_lens: &[usize]) -> Result<Self> {
        if word_lens.is_empty() || word_lens.len() > MAX_VOICE_WORDS {
            return Param::Words.range(1, MAX_VOICE_WORDS as i64, word_lens.len());
        }
        if word_lens.iter().any(|&l| l == 0 || l > 32) {
            return Param::WordLen.range(
                1,
                32,
                word_lens
                    .iter()
                    .copied()
                    .find(|&l| l == 0 || l > 32)
                    .unwrap_or(0),
            );
        }
        if word_lens.iter().sum::<usize>() != dest.len() {
            return Param::WordLen.conflict(Param::FrameLen);
        }
        let order = Permutation::from_table(dest)?;
        let mut lens = [0usize; MAX_VOICE_WORDS];
        lens[..word_lens.len()].copy_from_slice(word_lens);
        Ok(VoiceLayout {
            order,
            lens,
            count: word_lens.len(),
        })
    }

    /// Channel bits per frame.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    /// Always false: a layout has at least one bit.
    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The code word lengths.
    pub fn word_lens(&self) -> &[usize] {
        &self.lens[..self.count]
    }

    /// Puts the channel bits (or soft values) of one frame into code word order: `words[dest[i]] = channel[i]`. An error when
    /// a slice is not [`VoiceLayout::len`] long.
    pub fn split<T: Copy>(&self, channel: &[T], words: &mut [T]) -> Result<()> {
        Ok(self.order.apply(channel, words)?)
    }

    /// The inverse of [`VoiceLayout::split`]: the channel bits of one frame from its code words. An error when a slice is not
    /// [`VoiceLayout::len`] long.
    pub fn join<T: Copy>(&self, words: &[T], channel: &mut [T]) -> Result<()> {
        Ok(self.order.invert(words, channel)?)
    }

    /// Code word `k` of `words` (bits in code word order, one per byte) as a number, its first bit most significant. An error
    /// when `k` is not a word of the layout or `words` is shorter than the frame.
    pub fn word(&self, words: &[u8], k: usize) -> Result<u32> {
        if k >= self.count {
            return Param::Word.range(0, self.count as i64 - 1, k);
        }
        if words.len() < self.len() {
            return Err(FecError::InputLength {
                expected: self.len(),
                got: words.len(),
            });
        }
        let start: usize = self.lens[..k].iter().sum();
        Ok(words[start..start + self.lens[k]]
            .iter()
            .fold(0u32, |v, &b| (v << 1) | (b & 1) as u32))
    }
}

/// The AMBE+2 half-rate channel order that DMR, NXDN, dPMR and P25 Phase 2 share. Channel dibit i carries bit `X[i]` of
/// word `W[i]` and bit `Z[i]` of word `Y[i]`, where bit 0 is the last bit of a word.
const AMBE_HALF_RATE_W: [u8; 36] = [
    0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 2, 0, 2, 0, 2, 0, 2, 0, 2,
    0, 2, 0, 2,
];
const AMBE_HALF_RATE_X: [u8; 36] = [
    23, 10, 22, 9, 21, 8, 20, 7, 19, 6, 18, 5, 17, 4, 16, 3, 15, 2, 14, 1, 13, 0, 12, 10, 11, 9,
    10, 8, 9, 7, 8, 6, 7, 5, 6, 4,
];
const AMBE_HALF_RATE_Y: [u8; 36] = [
    0, 2, 0, 2, 0, 2, 0, 2, 0, 3, 0, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3, 1, 3,
    1, 3, 1, 3,
];
const AMBE_HALF_RATE_Z: [u8; 36] = [
    5, 3, 4, 2, 3, 1, 2, 0, 1, 13, 0, 12, 22, 11, 21, 10, 20, 9, 19, 8, 18, 7, 17, 6, 16, 5, 15, 4,
    14, 3, 13, 2, 12, 1, 11, 0,
];

impl VoiceLayout {
    /// The AMBE+2 half-rate layout (72 channel bits into the words C0 to C3 of [`AMBE_HALF_RATE_WORDS`]) that DMR, NXDN,
    /// dPMR and P25 Phase 2 share. The NXDN EHR test frames fix C0, C2 and C3 (see [`check_ambe_half_rate`]).
    /// Standard: NXDN TS 1-A Version 1.3 clause 7.2.1.
    pub fn ambe_half_rate() -> Self {
        let mut start = [0usize; 4];
        for k in 1..4 {
            start[k] = start[k - 1] + AMBE_HALF_RATE_WORDS[k - 1];
        }
        let place = |w: u8, b: u8| {
            (start[w as usize] + AMBE_HALF_RATE_WORDS[w as usize] - 1 - b as usize) as u16
        };
        let mut dest = [0u16; 72];
        for i in 0..36 {
            dest[2 * i] = place(AMBE_HALF_RATE_W[i], AMBE_HALF_RATE_X[i]);
            dest[2 * i + 1] = place(AMBE_HALF_RATE_Y[i], AMBE_HALF_RATE_Z[i]);
        }
        VoiceLayout::new(&dest, &AMBE_HALF_RATE_WORDS).expect("a permutation of 72 bits")
    }
}

/// Makes sure that an AMBE+2 half-rate layout agrees with the two NXDN EHR test frames ([`NXDN_EHR_TONE`],
/// [`NXDN_EHR_SILENCE`]): C0 must be a Golay (24,12) code word that holds information bits 0 to 11, C2 must be bits 24 to 34
/// and C3 bits 35 to 48. C1 is not checked, because the vocoder whitens it with a sequence no free standard provides. An error
/// when the layout does not have the [`AMBE_HALF_RATE_WORDS`] shape or disagrees with a frame.
pub fn check_ambe_half_rate(layout: &VoiceLayout) -> Result<()> {
    if layout.word_lens() != AMBE_HALF_RATE_WORDS {
        return Param::WordLen.conflict(Param::Layout);
    }
    let golay = SmallCode::golay_24_12();
    for f in [NXDN_EHR_TONE, NXDN_EHR_SILENCE] {
        let channel: [u8; 72] = std::array::from_fn(|i| f.coded_bit(i));
        let mut words = [0u8; 72];
        layout.split(&channel, &mut words)?;
        let info =
            |from: usize, to: usize| (from..to).fold(0u32, |v, i| (v << 1) | f.info_bit(i) as u32);
        let c0 = layout.word(&words, 0)?;
        let c0_ok = golay.syndrome(c0 as u128) == 0 && golay.info(c0 as u128) as u32 == info(0, 12);
        if !c0_ok
            || layout.word(&words, 2)? != info(24, 35)
            || layout.word(&words, 3)? != info(35, 49)
        {
            return Param::Layout.degenerate();
        }
    }
    Ok(())
}
