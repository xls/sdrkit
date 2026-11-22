//! D-STAR voice frame (JARL D-STAR, the coder of 2400 bit/s speech in 3600 bit/s): quantiser values b0..b8, the vectors
//! u0..u2, the words A, B and C, frame types and tone frames. The quantisers and the amplitude reconstruction are those
//! of [`crate::half`] with the tables [`BOOKS`].

use crate::fec::{self, codes};
use crate::half::{self, Books};
use crate::tables::dstar::{
    GAIN_LEVELS, HARMONICS, HOC_B5, HOC_B6, HOC_B7, HOC_B8, PRBA24, PRBA58, VOICING,
};
use std::f64::consts::PI;

/// Speech bits per frame.
pub const SPEECH_BITS: usize = 48;
/// Channel bits per frame.
pub const FRAME_BITS: usize = 72;
/// Lengths of the words A, B and C.
pub const WORDS: [usize; 3] = [24, 24, 24];
/// Lengths of u0, u1 and u2.
const INFO: [usize; 3] = [12, 12, 24];
/// Bits of b0..b8.
#[cfg(test)]
pub const WIDTHS: [u8; 9] = [7, 4, 6, 9, 7, 4, 4, 4, 3];

/// The largest b0 of a voice frame. The values 120 to 125 are voice frames of 56 harmonics, as the frame-type table of
/// the coder has them; the half-rate coder uses them for erasure and silence, and the silence pattern of JARL has
/// b0 = 124.
pub const B0_VOICE_MAX: u32 = 125;
/// The largest number of corrected errors in the data bits of words A and B together of a frame that is decoded; a frame
/// with more is repeated.
pub const REPEAT_ERRORS: u32 = 3;
/// Whether a word A with four errors, which the parity bit of the (24,12) code detects, makes the frame a repeat. Word B
/// is decoded as a (23,12) word and its bit 0 is not read.
pub const WORD_A_DETECTS: bool = true;

/// The silence pattern of JARL D-STAR (section 6.6: the voice frame of a lost packet), 9 bytes, each sent least
/// significant bit first.
pub const SILENCE: [u8; 9] = [0x9E, 0x8D, 0x32, 0x88, 0x26, 0x1A, 0x3F, 0x61, 0xE8];

/// The 72 channel bits of [`SILENCE`] in the order sent.
pub fn silence_bits() -> [u8; FRAME_BITS] {
    std::array::from_fn(|i| (SILENCE[i / 8] >> (i % 8)) & 1)
}

/// Bit positions of the 48 speech bits: entry `p` is (value, bit) of speech bit `p`, the speech bits being u0, u1, u2
/// each most significant bit first.
pub static LAYOUT: [(u8, u8); SPEECH_BITS] = [
    (0, 6), (0, 5), (0, 4), (0, 3), (0, 2), (0, 1), (2, 5), (2, 4), (2, 3), (2, 2), (3, 8), (3, 7),
    (3, 6), (3, 5), (3, 4), (3, 3), (3, 2), (4, 6), (4, 5), (4, 4), (4, 3), (4, 2), (5, 3), (5, 2),
    (5, 1), (5, 0), (6, 3), (6, 2), (6, 1), (6, 0), (7, 3), (7, 2), (7, 1), (7, 0), (8, 2), (8, 1),
    (8, 0), (1, 3), (1, 2), (1, 1), (1, 0), (2, 1), (2, 0), (3, 1), (3, 0), (4, 1), (4, 0), (0, 0),
];

/// omega_0 of b0: 2 pi 2^(-4.311767578125 - 0.021336 (b0 + 0.5)). No public document gives the pitch table of the
/// coder; this formula is the one in use among decoders and is not confirmed against a radio.
pub fn w0_of(b0: u32) -> f64 {
    2.0 * PI * (-4.311767578125 - 0.021336 * (b0.min(B0_VOICE_MAX) as f64 + 0.5)).exp2()
}

/// L of b0.
pub fn l_of(b0: u32) -> usize {
    HARMONICS[b0.min(B0_VOICE_MAX) as usize] as usize
}

/// The tables of the D-STAR coder. The encoder sends b0 = 0..119 and searches all 16 voicing vectors; b8 selects the
/// even rows of the block-4 codebook; the state starts at Lambda(-1) = 0, L(-1) = 30.
pub static BOOKS: Books = Books {
    w0: w0_of,
    l: l_of,
    voice_b0: 120,
    voicing: &VOICING,
    voicing_search: 16,
    gain: &GAIN_LEVELS,
    prba24: &PRBA24,
    prba58: &PRBA58,
    hoc: [&HOC_B5, &HOC_B6, &HOC_B7, &HOC_B8],
    hoc_step: [1, 1, 1, 2],
    start_l: 30,
    start_lambda: 0.0,
};

/// The 48 speech bits of b0..b8.
pub fn pack(b: &[u32; 9], out: &mut [u8; SPEECH_BITS]) {
    half::pack_fields(&LAYOUT, b, out);
}

/// b0..b8 of the 48 speech bits.
pub fn unpack(bits: &[u8; SPEECH_BITS], b: &mut [u32; 9]) {
    half::unpack_fields(&LAYOUT, bits, b);
}

/// A tone frame's index and level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    /// Tone index: 5..=122 a single tone of 31.25 Hz times the index, 128..=143 DTMF, 144..=146 call progress, 255 no
    /// tone.
    pub id: u8,
    /// Level, 8 bits; its scale is not known.
    pub level: u8,
}

/// Whether the speech bits are a tone frame: u0 bits 11..6 all ones, that is b0 = 126 or 127.
pub fn is_tone(bits: &[u8; SPEECH_BITS]) -> bool {
    bits[..6].iter().all(|&b| b & 1 == 1)
}

/// The tone index bits 7..5 for each 3-bit code in u0 bits 5..3. The codes 5 to 7 are not confirmed.
const TONE_HIGH: [u8; 8] = [4, 0, 1, 2, 3, 7, 6, 5];

/// The tone of a tone frame's speech bits. Index bits 7..5 are coded in u0 bits 5..3 ([`TONE_HIGH`]), bit 4 is u0 bit 2,
/// bits 3 and 2 are u2 bits 6 and 5, bits 1 and 0 are u0 bits 1 and 0. Level bits 7..3 are u1 bits 11..7, bits 2..0 are
/// u2 bit 4, u2 bit 3 and u1 bit 6. The places of level bits 4..0 are not confirmed.
pub fn tone_from(bits: &[u8; SPEECH_BITS]) -> Tone {
    let u = vectors_of(bits);
    let id = (TONE_HIGH[(u[0] >> 3 & 7) as usize] as u32) << 5
        | (u[0] >> 2 & 1) << 4
        | (u[2] >> 5 & 3) << 2
        | (u[0] & 3);
    let level = (u[1] >> 7 & 31) << 3 | (u[2] >> 3 & 3) << 1 | (u[1] >> 6 & 1);
    Tone {
        id: id as u8,
        level: level as u8,
    }
}

/// The speech bits of a tone frame, the inverse of [`tone_from`]; every other bit is 0.
pub fn tone_bits(t: Tone, out: &mut [u8; SPEECH_BITS]) {
    let (id, level) = (t.id as u32, t.level as u32);
    let code = TONE_HIGH.iter().position(|&h| h as u32 == id >> 5).unwrap_or(0) as u32;
    let u = [
        63 << 6 | code << 3 | (id >> 4 & 1) << 2 | (id & 3),
        (level >> 3) << 7 | (level & 1) << 6,
        (id >> 2 & 3) << 5 | (level >> 1 & 3) << 3,
    ];
    from_vectors(&u, out);
}

/// The 48 speech bits as u0..u2.
fn vectors_of(bits: &[u8; SPEECH_BITS]) -> [u32; 3] {
    let mut u = [0u32; 3];
    let mut p = 0;
    for (i, &n) in INFO.iter().enumerate() {
        u[i] = fec::word_of(&bits[p..p + n]);
        p += n;
    }
    u
}

/// The inverse of [`vectors_of`].
fn from_vectors(u: &[u32; 3], bits: &mut [u8; SPEECH_BITS]) {
    let mut p = 0;
    for (i, &n) in INFO.iter().enumerate() {
        fec::put_word(u[i], &mut bits[p..p + n]);
        p += n;
    }
}

/// What the error correction found in one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Errors {
    /// Bits corrected in the words A and B.
    pub corrected: u32,
    /// The corrected bits that are data bits of A and B.
    pub data: u32,
    /// Word A has more errors than its code corrects.
    pub beyond: bool,
}

impl Errors {
    /// Whether the frame is repeated instead of decoded ([`REPEAT_ERRORS`], [`WORD_A_DETECTS`]).
    pub fn repeats(&self) -> bool {
        self.data > REPEAT_ERRORS || (WORD_A_DETECTS && self.beyond)
    }
}

/// Word B as sent for the (23,12) code word `c1` and the vector u0 of the frame: the 23 bits XORed with the modulation
/// vector of u0, then bit 0, the even parity of those 23 bits as sent. The parity bit is not confirmed against a radio.
fn word_b(c1: u32, u0: u32) -> u32 {
    let sent = c1 ^ half::modulation_word(u0);
    sent << 1 | (sent.count_ones() & 1)
}

/// The words A, B and C of the 72 channel bits in the order sent.
fn words_of(chan: &[u8]) -> [u32; 3] {
    let mut words = [0u8; FRAME_BITS];
    let mut w = [0u32; 3];
    if codes().dstar_layout.split(&chan[..FRAME_BITS], &mut words).is_ok() {
        for (i, o) in w.iter_mut().enumerate() {
            *o = fec::word_of(&words[24 * i..24 * i + 24]);
        }
    }
    w
}

/// FEC-decodes the 72 channel bits, in the order sent, into the speech bits. Word A is a (24,12) Golay word. Word B bits
/// 23..1 are a (23,12) Golay word under the modulation vector of u0; its bit 0 is not read. Word C has no code. A word A
/// beyond the code's radius keeps its received data bits.
pub fn decode_channel(chan: &[u8], bits: &mut [u8; SPEECH_BITS]) -> Errors {
    let c = codes();
    let [a, b, u2] = words_of(chan);
    let mut e = Errors::default();
    let u0 = match c.golay24.decode(a as u128) {
        Some((cw, n)) => {
            e.corrected += n as u32;
            e.data += ((a ^ cw as u32) >> 12).count_ones();
            c.golay24.info(cw) as u32
        }
        None => {
            e.beyond = true;
            e.corrected += c.golay24.radius() as u32 + 1;
            a >> 12
        }
    };
    let c1 = b >> 1 ^ half::modulation_word(u0);
    let u1 = match c.golay23.decode(c1 as u128) {
        // With a wrong u0 the modulation vector is wrong and the corrections in word B mean nothing.
        Some((cw, n)) if !e.beyond => {
            e.corrected += n as u32;
            e.data += ((c1 ^ cw as u32) >> 11).count_ones();
            c.golay23.info(cw) as u32
        }
        _ => c1 >> 11,
    };
    from_vectors(&[u0, u1, u2], bits);
    e
}

/// The 72 channel bits of the speech bits, in the order sent.
pub fn encode_channel(bits: &[u8; SPEECH_BITS], chan: &mut [u8]) {
    let c = codes();
    let u = vectors_of(bits);
    let w = [
        c.golay24.encode(u[0] as u128) as u32,
        word_b(c.golay23.encode(u[1] as u128) as u32, u[0]),
        u[2],
    ];
    let mut words = [0u8; FRAME_BITS];
    for (i, &x) in w.iter().enumerate() {
        fec::put_word(x, &mut words[24 * i..24 * i + 24]);
    }
    let _ = c.dstar_layout.join(&words, &mut chan[..FRAME_BITS]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::half::{dequantise_amps, quantise_frame, voice_params, voicing_element, AmpState};
    use crate::model::Params;
    use crate::tables::half::BLOCKS;

    /// The 9 bytes, each sent least significant bit first, of the channel bits in the order sent.
    fn bytes_of(chan: &[u8; FRAME_BITS]) -> [u8; 9] {
        let mut out = [0u8; 9];
        for (i, &b) in chan.iter().enumerate() {
            out[i / 8] |= (b & 1) << (i % 8);
        }
        out
    }

    fn fields_of(chan: &[u8; FRAME_BITS]) -> ([u32; 9], Errors) {
        let mut sb = [0u8; SPEECH_BITS];
        let e = decode_channel(chan, &mut sb);
        let mut b = [0u32; 9];
        unpack(&sb, &mut b);
        (b, e)
    }

    fn rng(x: &mut u64) -> u32 {
        *x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (*x >> 33) as u32
    }

    fn close(got: &[f64], want: &[f64], tol: f64) {
        assert_eq!(got.len(), want.len());
        for (i, (g, w)) in got.iter().zip(want).enumerate() {
            assert!((g - w).abs() <= tol, "element {i}: {g} for {w}");
        }
    }

    #[test]
    fn tables_have_the_stated_sizes_and_sums() {
        // Every one of the 48 bits appears once, and every channel bit carries one word bit.
        let mut seen = [[false; 9]; 9];
        for &(v, i) in &LAYOUT {
            assert!(!seen[v as usize][i as usize]);
            seen[v as usize][i as usize] = true;
        }
        for (v, &w) in WIDTHS.iter().enumerate() {
            assert_eq!(seen[v].iter().filter(|&&s| s).count(), w as usize);
        }
        assert_eq!(WIDTHS.iter().map(|&w| w as usize).sum::<usize>(), SPEECH_BITS);
        // The harmonic counts: 9 to 56, never decreasing, equal to the half-rate table for the voice rows.
        assert_eq!(HARMONICS.iter().map(|&l| l as u32).sum::<u32>(), 3433);
        assert!(HARMONICS.windows(2).all(|p| p[0] <= p[1]));
        assert_eq!((HARMONICS[0], HARMONICS[125]), (9, 56));
        for b0 in 0..120 {
            assert_eq!(l_of(b0), half::l_of(b0));
        }
        for (n, row) in VOICING.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                assert_eq!(v as usize, n >> (3 - j / 2) & 1);
            }
        }
        let sum = |x: &[f64]| x.iter().sum::<f64>();
        assert!((sum(&GAIN_LEVELS) - 178.881538).abs() < 1e-6);
        assert!(GAIN_LEVELS.windows(2).all(|p| p[0] <= p[1]));
        assert_eq!(GAIN_LEVELS[62], GAIN_LEVELS[63]);
        let columns = |t: &[[f64; 4]]| -> [f64; 4] { std::array::from_fn(|c| t.iter().map(|r| r[c]).sum()) };
        let col3: [f64; 3] = std::array::from_fn(|c| PRBA24.iter().map(|r| r[c]).sum());
        close(&col3, &[92.6875, -4.375, 15.9375], 1e-6);
        close(&columns(&PRBA58), &[-1.523439, 0.757816, -2.320317, 0.757813], 1e-6);
        close(&columns(&HOC_B5), &[-1.570312, -0.687501, -0.734376, -0.234377], 1e-6);
        close(&columns(&HOC_B6), &[-0.093750, -0.101563, -0.062501, 0.101560], 1e-6);
        close(&columns(&HOC_B7), &[0.132811, 0.078124, -0.187500, 0.054685], 1e-6);
        close(&columns(&HOC_B8), &[-0.781252, 0.203123, -0.265625, 0.101561], 1e-6);
    }

    #[test]
    fn pitch_round_trips_and_the_voicing_index_stays_in_the_vector() {
        assert!((w0_of(0) / (2.0 * PI) - 0.04998242).abs() < 1e-8);
        assert!((w0_of(119) / (2.0 * PI) - 0.00860017).abs() < 1e-8);
        assert!((2.0 * PI / w0_of(125) - 127.066).abs() < 1e-3);
        for b0 in 0..=B0_VOICE_MAX {
            if b0 < 120 {
                assert_eq!(BOOKS.b0_of(w0_of(b0)), b0);
            }
            let exact = (16.0 * l_of(b0) as f64 * w0_of(b0) / (2.0 * PI)).floor();
            assert!(exact <= 7.0 && voicing_element(l_of(b0), w0_of(b0)) == exact as usize);
        }
    }

    #[test]
    fn the_silence_pattern_decodes_with_no_error_only_least_significant_bit_first() {
        let chan = silence_bits();
        assert_eq!(words_of(&chan), [0xF85334, 0x439A0E, 0x9DE80A]);
        let (b, e) = fields_of(&chan);
        assert_eq!(e, Errors::default());
        assert_eq!(b, [124, 0, 4, 165, 41, 10, 7, 7, 5]);
        // Re-encoding gives the same bytes, parity bit of word B included.
        let mut sb = [0u8; SPEECH_BITS];
        pack(&b, &mut sb);
        let mut back = [0u8; FRAME_BITS];
        encode_channel(&sb, &mut back);
        assert_eq!(bytes_of(&back), SILENCE);
        // The other bit order has three errors in each word.
        let msb: [u8; FRAME_BITS] = std::array::from_fn(|i| (SILENCE[i / 8] >> (7 - i % 8)) & 1);
        let (_, e) = fields_of(&msb);
        assert_eq!(e.corrected, 6);
    }

    #[test]
    fn the_silence_pattern_gives_the_printed_amplitudes() {
        let (b, _) = fields_of(&silence_bits());
        let mut p = Params::silent(0.0, 0);
        voice_params(&BOOKS, b[0], b[1], &mut p);
        assert_eq!(p.l, 56);
        assert!((p.w0 - 0.050185).abs() < 1e-6);
        assert!(p.voiced.iter().all(|&v| !v));
        let st = BOOKS.start();
        let mut next = st;
        dequantise_amps(&BOOKS, &b, &st, &mut next, &mut p);
        assert!((next.gamma - 0.590088).abs() < 1e-9);
        close(
            &[next.lambda[1], next.lambda[2], next.lambda[14], next.lambda[56]],
            &[-2.4794, -2.3934, -1.9222, -2.2192],
            1e-4,
        );
        close(&[p.amp[1], p.amp[56]], &[0.1638, 0.1962], 1e-4);
    }

    #[test]
    fn the_worked_voice_frame_packs_codes_and_decodes_as_printed() {
        let b = [60, 12, 40, 300, 77, 9, 3, 12, 3];
        let mut sb = [0u8; SPEECH_BITS];
        pack(&b, &mut sb);
        assert_eq!(vectors_of(&sb), [0x7AA, 0x5CE, 0x4F1E02]);
        assert_eq!(half::modulation_word(0x7AA), 0x2FF980);
        let mut chan = [0u8; FRAME_BITS];
        encode_channel(&sb, &mut chan);
        assert_eq!(words_of(&chan), [0x7AAF08, 0x0312A3, 0x4F1E02]);
        assert_eq!(bytes_of(&chan), [0x47, 0x3D, 0x70, 0x98, 0x2B, 0x8A, 0xC0, 0x09, 0x4E]);
        assert_eq!(fields_of(&chan), (b, Errors::default()));

        let mut p = Params::silent(0.0, 0);
        voice_params(&BOOKS, b[0], b[1], &mut p);
        assert_eq!((p.l, BLOCKS[p.l - 9]), (23, [5, 5, 6, 7]));
        assert!((p.w0 - 0.129310).abs() < 1e-6);
        assert!((1..=23).all(|l| p.voiced[l] == (l <= 12)));
        let st = BOOKS.start();
        let mut first = st;
        dequantise_amps(&BOOKS, &b, &st, &mut first, &mut p);
        assert!((first.gamma - 3.407471).abs() < 1e-9);
        close(
            &first.lambda[1..=23],
            &[
                1.5859, 2.9221, 1.3556, 1.4654, 1.0897, 0.8096, 1.1394, 1.2685, 0.9574, 0.4487, 1.5726, 1.8858, 0.7826,
                0.6799, 1.5387, 1.1890, 1.1409, 0.7521, 0.4808, 1.1054, 1.4007, 0.6480, 0.1318,
            ],
            1e-4,
        );
        close(
            &p.amp[1..=23],
            &[
                3.0012, 7.5764, 2.5586, 2.7609, 2.1280, 1.7525, 2.2025, 2.4086, 1.9416, 1.3647, 2.9736, 3.6946, 0.9787,
                0.9114, 1.6527, 1.2970, 1.2545, 0.9582, 0.7940, 1.2240, 1.5019, 0.8915, 0.6234,
            ],
            2e-4,
        );
        // The same frame again predicts from the first.
        let mut second = first;
        dequantise_amps(&BOOKS, &b, &first, &mut second, &mut p);
        assert!((second.gamma - 5.111206).abs() < 1e-6);
        close(&[second.lambda[1], second.lambda[2], second.lambda[23]], &[3.5757, 5.7806, 1.1765], 1e-4);
    }

    #[test]
    fn errors_count_in_the_data_bits_and_word_a_detects_four() {
        let b = [60, 12, 40, 300, 77, 9, 3, 12, 3];
        let mut sb = [0u8; SPEECH_BITS];
        pack(&b, &mut sb);
        let mut clean = [0u8; FRAME_BITS];
        encode_channel(&sb, &mut clean);
        let flipped = |at: &[usize]| {
            let mut c = clean;
            for &i in at {
                c[i] ^= 1;
            }
            fields_of(&c)
        };
        // Channel bit 0 is A10, a parity bit.
        let (got, e) = flipped(&[0]);
        assert_eq!((got, e.corrected, e.data, e.repeats()), (b, 1, 0, false));
        // Channel bits 32 to 35 are B05, B17, A05, A17: two errors a word, one of them in the data.
        let (got, e) = flipped(&[32, 33, 34, 35]);
        assert_eq!((got, e.corrected, e.data, e.repeats()), (b, 4, 2, false));
        // Three data bits of each word (A23, A22, A21 and B23, B22, B21): corrected, and the frame repeats.
        let (got, e) = flipped(&[7, 1, 11, 5, 15, 9]);
        assert_eq!((got, e.corrected, e.data, e.repeats()), (b, 6, 6, true));
        // Four errors in the data bits over both words repeat; three do not.
        let (got, e) = flipped(&[7, 1, 11, 5]);
        assert_eq!((got, e.data, e.repeats()), (b, 4, true));
        let (got, e) = flipped(&[7, 1, 11]);
        assert_eq!((got, e.data, e.repeats()), (b, 3, false));
        // Four errors in word A (A23, A22, A21, A20) are beyond the code and are detected.
        let (_, e) = flipped(&[7, 1, 11, 21]);
        assert!(e.beyond && e.repeats());
        // Bit 0 of word B (channel bit 66) is not read.
        let (got, e) = flipped(&[66]);
        assert_eq!((got, e), (b, Errors::default()));
        // Four errors in the 23 bits of word B give a wrong code word within three bits: it cannot be detected.
        let (got, e) = flipped(&[5, 15, 9, 19]);
        assert!(got != b && e.corrected <= 3 && !e.beyond);
    }

    #[test]
    fn random_fields_round_trip_through_the_channel() {
        let mut x = 11u64;
        for _ in 0..2000 {
            let mut b = [0u32; 9];
            for (v, &w) in WIDTHS.iter().enumerate() {
                b[v] = rng(&mut x) & ((1 << w) - 1);
            }
            let mut sb = [0u8; SPEECH_BITS];
            pack(&b, &mut sb);
            let mut chan = [0u8; FRAME_BITS];
            encode_channel(&sb, &mut chan);
            // Up to three errors in each of the words A and B are corrected.
            for w in 0..2 {
                for _ in 0..rng(&mut x) % 4 {
                    let bit = rng(&mut x) as usize % 24;
                    let at = (0..72).find(|&i| {
                        let mut one = [0u8; 72];
                        one[i] = 1;
                        words_of(&one)[w] == 1 << bit
                    });
                    chan[at.unwrap()] ^= 1;
                }
            }
            let (got, e) = fields_of(&chan);
            assert_eq!(got, b);
            assert!(!e.beyond && e.corrected <= 6);
        }
    }

    #[test]
    fn tone_frames_match_the_worked_example_and_the_printed_frames() {
        let mut sb = [0u8; SPEECH_BITS];
        tone_bits(Tone { id: 32, level: 64 }, &mut sb);
        assert_eq!(vectors_of(&sb), [0xFD0, 0x400, 0]);
        let mut chan = [0u8; FRAME_BITS];
        encode_channel(&sb, &mut chan);
        assert_eq!(bytes_of(&chan), [0xC3, 0x8F, 0x38, 0xE2, 0x86, 0x38, 0x11, 0x05, 0x0C]);
        let mut back = [0u8; SPEECH_BITS];
        assert_eq!(decode_channel(&chan, &mut back), Errors::default());
        assert!(is_tone(&back));
        assert_eq!(tone_from(&back), Tone { id: 32, level: 64 });
        // Received tone frames: u0, u1, bit 0 of word B, u2.
        for (text, id, level) in [
            ("1111110101001110100000001000000000000000001100000", 60, 232),
            ("1111110110101110100000000000000000000000000000000", 66, 232),
            ("1111110010101110110000001000000000000000000110000", 6, 237),
            ("1111110010011110100000001000000000000000000101000", 5, 234),
            ("1111110010011110000000001000000000000000000101000", 5, 226),
        ] {
            let all: Vec<u8> = text.bytes().map(|c| c - b'0').collect();
            let mut sb = [0u8; SPEECH_BITS];
            sb[..24].copy_from_slice(&all[..24]);
            sb[24..].copy_from_slice(&all[25..]);
            assert!(is_tone(&sb));
            let t = tone_from(&sb);
            assert_eq!((t.id, t.level), (id, level), "{text}");
            let mut again = [0u8; SPEECH_BITS];
            tone_bits(t, &mut again);
            assert_eq!(again, sb, "{text}");
            // The parity bit of word B as sent.
            encode_channel(&sb, &mut chan);
            assert_eq!(words_of(&chan)[1] & 1, all[24] as u32, "{text}");
        }
        for id in 0..=255u8 {
            let t = Tone { id, level: id.wrapping_mul(7) };
            tone_bits(t, &mut sb);
            assert!(is_tone(&sb));
            assert_eq!(tone_from(&sb), t);
        }
    }

    #[test]
    fn the_amplitude_loop_converges_and_the_encoder_state_is_the_decoder_state() {
        let b0 = 70;
        let mut p = Params::silent(w0_of(b0), l_of(b0));
        for li in 1..=p.l {
            p.amp[li] = 2000.0 / li as f64;
        }
        let bands = [true; 13];
        let (mut enc, mut dec): (AmpState, AmpState) = (BOOKS.start(), BOOKS.start());
        let mut b = [0u32; 9];
        let mut rec = Params::silent(0.0, 0);
        let mut err = 0.0;
        for _ in 0..20 {
            quantise_frame(&BOOKS, b0, &p, &bands, &mut enc, &mut b, &mut rec);
            assert!(b[8] < 8 && b[5] < 16 && b[1] == 15);
            let mut out = Params::silent(0.0, 0);
            voice_params(&BOOKS, b[0], b[1], &mut out);
            let mut next = dec;
            dequantise_amps(&BOOKS, &b, &dec, &mut next, &mut out);
            dec = next;
            assert_eq!(rec, out);
            assert_eq!((enc.lambda, enc.l, enc.gamma), (dec.lambda, dec.l, dec.gamma));
            err = (1..=p.l).map(|li| (out.amp[li] / p.amp[li]).log2().abs()).sum::<f64>() / p.l as f64;
        }
        assert!(err < 0.35, "mean log2 error {err}");
    }
}
