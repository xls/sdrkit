//! The TIP minor frame of the NOAA KLM satellites and the HIRS data inside it. The same frames arrive on the 137 MHz beacon
//! and inside the HRPT minor frame.
//!
//! Standard: NOAA KLM User's Guide, sections 4.3 (TIP), 4.3.4.1 (HIRS digital data) and 4.1.3 (TIP words of an HRPT frame).

use sdrkit_codec_image::strip::Strip;
use std::sync::Arc;

/// Bytes of a minor frame.
pub const TIP_BYTES: usize = 104;
/// Bits of a minor frame.
pub const TIP_BITS: usize = 8 * TIP_BYTES;
/// The first 16 bits of the frame sync (the full word has 20 bits: ED E2 0).
pub const TIP_SYNC: u16 = 0xEDE2;
/// Bits a second.
pub const TIP_BIT_RATE: f64 = 8320.0;
/// Minor frames of a major frame.
pub const MAJOR_FRAME: u16 = 320;
/// The bytes of a minor frame that hold the 36 bytes of a HIRS element, in order.
pub const HIRS_BYTES: [usize; 36] = [
    16, 17, 22, 23, 26, 27, 30, 31, 34, 35, 38, 39, 42, 43, 54, 55, 58, 59, 62, 63, 66, 67, 70, 71,
    74, 75, 78, 79, 82, 83, 84, 85, 88, 89, 92, 93,
];
/// The HIRS channel (1 to 20) of each of the 20 radiance words of an element, in the order they are sent.
pub const HIRS_CHANNELS: [u8; 20] = [
    1, 17, 2, 3, 13, 4, 18, 11, 19, 7, 8, 20, 10, 14, 6, 5, 15, 12, 16, 9,
];
/// Earth views of a HIRS line (elements 0 to 55; 56 to 63 are the retrace).
pub const HIRS_WIDTH: usize = 56;
/// Elements of a HIRS line.
pub const HIRS_ELEMENTS: u8 = 64;
/// The first word (from 0) of an HRPT minor frame that holds TIP data.
pub const HRPT_TIP_WORD: usize = 103;
/// The most HIRS lines of a picture.
pub const HIRS_MAX_ROWS: u32 = 4000;

/// The fields of the start of a minor frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TipHeader {
    /// The spacecraft address, 4 bits.
    pub spacecraft: u8,
    /// The minor frame counter, 0 to 319.
    pub minor_frame: u16,
}

/// Reads the header of a minor frame of at least 6 bytes.
pub fn tip_header(frame: &[u8]) -> TipHeader {
    TipHeader {
        spacecraft: frame[2] & 0x0F,
        minor_frame: (u16::from(frame[4] & 1) << 8) | u16::from(frame[5]),
    }
}

/// One HIRS element: the 288 bits that a minor frame holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HirsElement {
    /// The scan encoder position: the place of the scan mirror in steps of 1.8 degrees.
    pub encoder: u8,
    /// The element number: 0 to 55 are the Earth views of a line, 56 to 63 the retrace.
    pub element: u8,
    /// The radiometric data are valid.
    pub valid: bool,
    /// The 20 radiance words in sending order, each as 4096 plus its signed value (0 to 8191).
    pub words: [u16; 20],
    /// The element has odd parity over its 288 bits, as the instrument sends it.
    pub parity_ok: bool,
}

/// The 36 HIRS bytes of a minor frame.
pub fn hirs_bytes(frame: &[u8]) -> [u8; 36] {
    let mut h = [0u8; 36];
    for (o, &i) in h.iter_mut().zip(&HIRS_BYTES) {
        *o = frame[i];
    }
    h
}

fn bits(h: &[u8; 36], from: usize, n: usize) -> u16 {
    (from..from + n).fold(0, |v, i| {
        (v << 1) | u16::from((h[i / 8] >> (7 - i % 8)) & 1)
    })
}

/// Decodes the 36 HIRS bytes of a minor frame. Every radiance word is sign and magnitude: the first bit is the sign (1 is
/// positive), 12 bits of amplitude follow.
pub fn decode_hirs(h: &[u8; 36]) -> HirsElement {
    let mut words = [0u16; 20];
    for (w, o) in words.iter_mut().enumerate() {
        let raw = bits(h, 26 + 13 * w, 13);
        let magnitude = raw & 0x0FFF;
        *o = if raw & 0x1000 != 0 {
            4096 + magnitude
        } else {
            4096 - magnitude
        };
    }
    let ones: u32 = h.iter().map(|b| b.count_ones()).sum();
    HirsElement {
        encoder: h[0],
        element: bits(h, 19, 6) as u8,
        valid: bits(h, 286, 1) == 1,
        words,
        parity_ok: ones % 2 == 1,
    }
}

/// The 36 HIRS bytes of an element whose radiance words have the signed values `values` (-4095 to 4095), with odd parity.
pub fn encode_hirs(encoder: u8, element: u8, valid: bool, values: &[i16; 20]) -> [u8; 36] {
    let mut h = [0u8; 36];
    let mut put = |from: usize, n: usize, v: u16| {
        for k in 0..n {
            let i = from + k;
            h[i / 8] |= (((v >> (n - 1 - k)) & 1) as u8) << (7 - i % 8);
        }
    };
    put(0, 8, u16::from(encoder));
    put(19, 6, u16::from(element & 63));
    for (w, &v) in values.iter().enumerate() {
        let magnitude = v.unsigned_abs().min(4095);
        put(26 + 13 * w, 13, (u16::from(v >= 0) << 12) | magnitude);
    }
    put(286, 1, u16::from(valid));
    let ones: u32 = h.iter().map(|b| b.count_ones()).sum();
    h[35] |= u8::from(ones.is_multiple_of(2));
    h
}

/// The five TIP minor frames inside an HRPT minor frame of at least 623 words: byte `j` of frame `k` is the top 8 bits of
/// word 103 + 104 k + j. Only the HRPT minor frame with [`hrpt_minor_frame_number`] 1 holds TIP data.
pub fn tip_from_hrpt(words: &[u16]) -> Option<[[u8; TIP_BYTES]; 5]> {
    if words.len() < HRPT_TIP_WORD + 5 * TIP_BYTES {
        return None;
    }
    let mut frames = [[0u8; TIP_BYTES]; 5];
    for (k, f) in frames.iter_mut().enumerate() {
        for (j, b) in f.iter_mut().enumerate() {
            *b = (words[HRPT_TIP_WORD + TIP_BYTES * k + j] >> 2) as u8;
        }
    }
    Some(frames)
}

/// The minor frame number of an HRPT frame from its identification word (word 6 from 0): 1 holds TIP data, 2 a spare
/// pattern, 3 AMSU data.
pub fn hrpt_minor_frame_number(id_word: u16) -> u8 {
    ((id_word >> 7) & 3) as u8
}

/// The spacecraft address of an HRPT frame from its identification word.
pub fn hrpt_spacecraft(id_word: u16) -> u8 {
    ((id_word >> 3) & 15) as u8
}

/// Counters of [`HirsDecoder`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HirsStats {
    /// Elements with valid data that were put into a line.
    pub valid: u64,
    /// Elements without odd parity.
    pub parity_errors: u64,
    /// Lines given out.
    pub lines: u64,
    /// Rows of the picture.
    pub rows: u32,
}

/// Gathers HIRS elements into lines and gives each line out as 20 strips of 56 pixels, one for each channel. Element `e` is
/// column `e`: the columns are in the order of the scan. A pixel is the stored 13-bit value without its low 5 bits.
#[derive(Clone, Debug)]
pub struct HirsDecoder {
    line: [[u8; HIRS_WIDTH]; 20],
    filled: u32,
    /// Counters.
    pub stats: HirsStats,
}

impl Default for HirsDecoder {
    fn default() -> Self {
        HirsDecoder {
            line: [[0; HIRS_WIDTH]; 20],
            filled: 0,
            stats: HirsStats::default(),
        }
    }
}

impl HirsDecoder {
    /// Starts a new picture.
    pub fn new_picture(&mut self) {
        self.stats.rows = 0;
        self.filled = 0;
        self.line = [[0; HIRS_WIDTH]; 20];
    }

    /// Takes the element of one minor frame. A line is complete when element 55 arrives.
    pub fn push(&mut self, e: &HirsElement, out: &mut Vec<Strip>) {
        self.stats.parity_errors += u64::from(!e.parity_ok);
        let column = usize::from(e.element);
        if column < HIRS_WIDTH && e.valid {
            for (w, &v) in e.words.iter().enumerate() {
                let channel = usize::from(HIRS_CHANNELS[w]) - 1;
                self.line[channel][column] = (v >> 5).min(255) as u8;
            }
            self.filled += 1;
            self.stats.valid += 1;
        }
        if column == HIRS_WIDTH - 1 && self.filled > 0 {
            let row = self.stats.rows;
            if row < HIRS_MAX_ROWS {
                for (c, px) in self.line.iter().enumerate() {
                    let mut s = Strip::new(
                        c as u16 + 1,
                        row,
                        0,
                        1,
                        HIRS_WIDTH as u32,
                        Arc::from(&px[..]),
                    );
                    s.instrument = "HIRS";
                    out.push(s);
                }
                self.stats.rows += 1;
            }
            self.stats.lines += 1;
            self.filled = 0;
            self.line = [[0; HIRS_WIDTH]; 20];
        }
    }

    /// Takes a whole minor frame of 104 bytes.
    pub fn push_frame(&mut self, frame: &[u8], out: &mut Vec<Strip>) {
        if frame.len() >= TIP_BYTES {
            self.push(&decode_hirs(&hirs_bytes(frame)), out);
        }
    }
}

/// A minor frame of the test signal: the sync, the spacecraft address, the frame counter, one HIRS element and 0x55 in every
/// other byte. Elements 0 to 55 are valid Earth views at encoder position element + 1 whose word `w` has the value
/// `value(w, element, line)`; elements 56 to 63 are the retrace, not valid.
pub fn test_frame(
    spacecraft: u8,
    counter: u16,
    element: u8,
    line: u32,
    value: impl Fn(u16, u32, u32) -> u16,
) -> [u8; TIP_BYTES] {
    let mut f = [0x55u8; TIP_BYTES];
    f[0] = (TIP_SYNC >> 8) as u8;
    f[1] = TIP_SYNC as u8;
    f[2] = spacecraft & 0x0F;
    f[3] = 0;
    f[4] = ((counter % MAJOR_FRAME) >> 8) as u8;
    f[5] = (counter % MAJOR_FRAME) as u8;
    let earth = usize::from(element) < HIRS_WIDTH;
    let mut values = [0i16; 20];
    if earth {
        for (w, v) in values.iter_mut().enumerate() {
            *v = value(w as u16, u32::from(element), line) as i16;
        }
    }
    let encoder = if earth { element + 1 } else { 0 };
    let h = encode_hirs(encoder, element, earth, &values);
    for (&i, &b) in HIRS_BYTES.iter().zip(&h) {
        f[i] = b;
    }
    f
}
