//! The GAC frame of the NOAA KLM satellites: the recorder playback of reduced AVHRR data (409 pixels a channel) with five TIP
//! minor frames, 3327 words of 10 bits a frame, scrambled after the sync words by the inverted 1023-bit sequence.
//!
//! Standard: NOAA KLM User's Guide, sections 1.2.5, 1.2.6 and 3.1.3.2 (GAC), Table 4.1.3.1-1 (sync and identification
//! words, sequence generator).

use super::tip::{HirsDecoder, HirsStats, TIP_BYTES};
use sdrkit_codec_image::strip::Strip;
use sdrkit_util_bits::lfsr::Lfsr;
use std::sync::Arc;

/// Words of a frame.
pub const WORDS: usize = 3327;
/// Bits of a frame.
pub const FRAME_BITS: usize = 10 * WORDS;
/// Bytes of a frame with zero bits up to a whole byte.
pub const FRAME_BYTES: usize = FRAME_BITS.div_ceil(8);
/// Bits a second of the playback.
pub const BIT_RATE: f64 = 2_661_600.0;
/// The six sync words at the start of a frame.
pub const SYNC_WORDS: [u16; 6] = [0x284, 0x16F, 0x35C, 0x19D, 0x20F, 0x095];
/// The first 32 bits of the sync words: the word the receiver searches.
pub const SYNC: u32 = 0xA116_FD71;
/// The first scrambled bit: the first bit of word 6.
pub const SCRAMBLED_FROM: usize = 60;
/// The length of the sequence.
pub const SEQUENCE_BITS: usize = 1023;
/// The identification word.
pub const ID_WORD: usize = 6;
/// The first word of the five TIP minor frames.
pub const TIP_WORD: usize = 103;
/// The first AVHRR word.
pub const AVHRR_WORD: usize = 1182;
/// Pixels of a line.
pub const WIDTH: usize = 409;
/// AVHRR channels.
pub const CHANNELS: usize = 5;
/// The most rows of the picture.
pub const MAX_ROWS: u32 = 50_000;
/// The instrument name of the AVHRR strips.
pub const INSTRUMENT: &str = "AVHRR GAC";

/// The 1023 bits of the scrambling sequence: the inverted output of the generator x^10 + x^5 + x^2 + x + 1 from the
/// all-ones state, whose first ten output bits are 1111100010.
pub fn sequence() -> Vec<u8> {
    // u(n) = u(n - 10) ^ u(n - 9) ^ u(n - 8) ^ u(n - 5), with u(0) to u(9) = 1 1 1 1 1 0 0 0 1 0.
    let mut g = Lfsr::recurrence(10, 0b10_0111, 0b01_0001_1111);
    let mut s = vec![0u8; SEQUENCE_BITS];
    g.fill_bits(&mut s);
    s.iter_mut().for_each(|b| *b ^= 1);
    s
}

/// The scrambling of a frame as a byte mask: frame bit `b` (60 or more) is XORed with `s((b - 60) mod 1023)`.
pub fn scramble_mask() -> Vec<u8> {
    let s = sequence();
    let mut m = vec![0u8; FRAME_BYTES];
    for b in SCRAMBLED_FROM..FRAME_BITS {
        m[b / 8] |= s[(b - SCRAMBLED_FROM) % SEQUENCE_BITS] << (7 - b % 8);
    }
    m
}

/// Word `w` of a frame of bytes.
pub fn word(frame: &[u8], w: usize) -> u16 {
    let bit = 10 * w;
    let at = bit / 8;
    let v = u32::from(*frame.get(at).unwrap_or(&0)) << 16
        | u32::from(*frame.get(at + 1).unwrap_or(&0)) << 8
        | u32::from(*frame.get(at + 2).unwrap_or(&0));
    ((v >> (14 - bit % 8)) & 0x3FF) as u16
}

/// The bytes of 3327 words, most significant bit first, with zero bits up to a whole byte.
pub fn pack_words(words: &[u16]) -> Vec<u8> {
    let mut out = vec![0u8; FRAME_BYTES];
    for (w, &v) in words.iter().take(WORDS).enumerate() {
        for k in 0..10 {
            let b = 10 * w + k;
            out[b / 8] |= (((v >> (9 - k)) & 1) as u8) << (7 - b % 8);
        }
    }
    out
}

/// The frame code of the identification word: bits 1 and 2. A GAC frame has code 0.
pub fn frame_code(id: u16) -> u8 {
    ((id >> 7) & 3) as u8
}

/// The spacecraft address of the identification word: bits 3 to 6.
pub fn spacecraft(id: u16) -> u8 {
    ((id >> 3) & 15) as u8
}

/// The five TIP minor frames of a frame of words: byte j of frame k is the top 8 bits of word 103 + 104 k + j.
pub fn tip_frames(words: &[u16]) -> [[u8; TIP_BYTES]; 5] {
    let mut t = [[0u8; TIP_BYTES]; 5];
    for (k, f) in t.iter_mut().enumerate() {
        for (j, b) in f.iter_mut().enumerate() {
            *b = (words
                .get(TIP_WORD + TIP_BYTES * k + j)
                .copied()
                .unwrap_or(0)
                >> 2) as u8;
        }
    }
    t
}

/// The test frame of line `y`: the sync words, the identification word of `scid` with frame code 0, the five TIP frames
/// `tip` (each word a TIP byte times 4), pixel x of channel c equal to `value(c, x, y)`, every other word 0. Not scrambled.
pub fn test_words(
    scid: u8,
    y: u32,
    tip: &[[u8; TIP_BYTES]; 5],
    value: impl Fn(u16, u32, u32) -> u16,
) -> Vec<u16> {
    let mut w = vec![0u16; WORDS];
    w[..6].copy_from_slice(&SYNC_WORDS);
    w[ID_WORD] = u16::from(scid & 15) << 3;
    for (k, f) in tip.iter().enumerate() {
        for (j, &b) in f.iter().enumerate() {
            w[TIP_WORD + TIP_BYTES * k + j] = u16::from(b) << 2;
        }
    }
    for x in 0..WIDTH {
        for c in 0..CHANNELS {
            w[AVHRR_WORD + CHANNELS * x + c] = value(c as u16, x as u32, y);
        }
    }
    w
}

/// Counters of [`GacDecoder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GacStats {
    /// Frames.
    pub frames: u64,
    /// TIP minor frames.
    pub tip_frames: u64,
    /// Rows of the AVHRR picture.
    pub rows: u32,
    /// The HIRS counters.
    pub hirs: HirsStats,
}

/// Descrambled frames to AVHRR and HIRS strips. One AVHRR row a frame, in the order the frames arrive.
#[derive(Clone, Debug)]
pub struct GacDecoder {
    mask: Vec<u8>,
    words: Vec<u16>,
    hirs: HirsDecoder,
    /// The identification word of the last frame.
    pub id: Option<u16>,
    /// Counters.
    pub stats: GacStats,
}

impl Default for GacDecoder {
    fn default() -> Self {
        GacDecoder {
            mask: scramble_mask(),
            words: vec![0; WORDS],
            hirs: HirsDecoder::default(),
            id: None,
            stats: GacStats::default(),
        }
    }
}

impl GacDecoder {
    /// Starts new pictures.
    pub fn new_picture(&mut self) {
        self.stats.rows = 0;
        self.hirs.new_picture();
    }

    /// The scrambling mask of a frame.
    pub fn mask(&self) -> &[u8] {
        &self.mask
    }

    /// Takes a frame as the synchroniser gave it (scrambled) and appends its strips to `out`.
    pub fn push(&mut self, frame: &[u8], out: &mut Vec<Strip>) {
        let mut f = frame[..FRAME_BYTES.min(frame.len())].to_vec();
        f.iter_mut().zip(&self.mask).for_each(|(b, m)| *b ^= m);
        for (w, v) in self.words.iter_mut().enumerate() {
            *v = word(&f, w);
        }
        self.stats.frames += 1;
        self.id = Some(self.words[ID_WORD]);
        for t in tip_frames(&self.words) {
            self.hirs.push_frame(&t, out);
            self.stats.tip_frames += 1;
        }
        self.stats.hirs = self.hirs.stats;
        if self.stats.rows < MAX_ROWS {
            for c in 0..CHANNELS {
                let px: Vec<u8> = (0..WIDTH)
                    .map(|x| (self.words[AVHRR_WORD + CHANNELS * x + c] >> 2) as u8)
                    .collect();
                let mut s = Strip::new(
                    c as u16 + 1,
                    self.stats.rows,
                    0,
                    1,
                    WIDTH as u32,
                    Arc::from(px),
                );
                s.instrument = INSTRUMENT;
                out.push(s);
            }
            self.stats.rows += 1;
        }
    }
}
