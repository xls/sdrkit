//! MSU-GS frames of the Elektro-L and Arktika-M raw downlink: infrared frames of 1820 bytes (one segment of 183 pixels of
//! each of 7 channels) and visible frames of 15 210 bytes (one line of 6004 pixels of one channel). Ten-bit words are packed
//! four to five bytes, the first word in the top bits. No public document gives these layouts.

use crate::instruments::avhrr::{pack10, word10};

/// The 64-bit sync word at the start of every frame.
pub const SYNC: u64 = 0x0218_A7A3_92DD_9ABF;
/// Bytes of an infrared frame.
pub const IR_BYTES: usize = 1820;
/// Bytes of a visible frame.
pub const VIS_BYTES: usize = 15_210;
/// Infrared channels.
pub const IR_CHANNELS: usize = 7;
/// Pixels of one channel in one infrared frame.
pub const IR_SEGMENT: usize = 183;
/// Infrared segments of a line (the even segment numbers 0 to 34).
pub const IR_SEGMENTS: usize = 18;
/// Columns of the infrared picture.
pub const IR_WIDTH: usize = IR_SEGMENTS * IR_SEGMENT;
/// Lines of the infrared picture.
pub const IR_LINES: u32 = 3400;
/// The first word of each infrared channel (words counted from byte 5).
pub const IR_OFFSETS: [usize; IR_CHANNELS] = [113, 305, 497, 689, 881, 1073, 1265];
/// Pixels of a visible line.
pub const VIS_PIXELS: usize = 6004;
/// Lines of the full visible picture.
pub const VIS_LINES: u32 = 17_200;
/// The reduction of the visible picture for the screen, in both directions.
pub const VIS_REDUCE: u32 = 4;
/// Columns of the reduced visible picture.
pub const VIS_WIDTH: usize = 1501;
/// The first byte of the visible words.
pub const VIS_WORDS_AT: usize = 190;
/// The first byte of the infrared words.
pub const IR_WORDS_AT: usize = 5;

/// A decoded infrared frame.
#[derive(Clone, Debug, PartialEq)]
pub struct IrFrame {
    /// The line counter (13 bits).
    pub line: u32,
    /// The segment number (6 bits).
    pub segment: u32,
    /// The segment number is even and below 36, and the line below 3400.
    pub valid: bool,
    /// 7 channels of 183 10-bit pixels, channel-major.
    pub pixels: Vec<u16>,
}

/// Decodes an infrared frame of at least [`IR_BYTES`] bytes (sync word first). `None` for a short frame.
pub fn decode_ir(f: &[u8]) -> Option<IrFrame> {
    if f.len() < IR_BYTES {
        return None;
    }
    let line = u32::from(f[8] & 0x1F) << 8 | u32::from(f[9]);
    let segment = u32::from(f[10] & 0x3F);
    let valid = segment.is_multiple_of(2) && segment < 36 && line < IR_LINES;
    let mut pixels = Vec::with_capacity(IR_CHANNELS * IR_SEGMENT);
    for o in IR_OFFSETS {
        pixels.extend((0..IR_SEGMENT).map(|i| word10(f, IR_WORDS_AT, o + i)));
    }
    Some(IrFrame {
        line,
        segment,
        valid,
        pixels,
    })
}

/// The infrared frame of `line` and `segment` with `pixels` (7 x 183 10-bit values, channel-major): the generator's side.
pub fn build_ir(line: u32, segment: u32, pixels: &[u16]) -> Vec<u8> {
    let words_len = IR_OFFSETS[IR_CHANNELS - 1] + IR_SEGMENT;
    let mut words = vec![0u16; words_len];
    for (c, o) in IR_OFFSETS.iter().enumerate() {
        for i in 0..IR_SEGMENT {
            words[o + i] = pixels[c * IR_SEGMENT + i] & 0x3FF;
        }
    }
    let mut f = vec![0u8; IR_BYTES];
    let packed = pack10(&words);
    let n = packed.len().min(IR_BYTES - IR_WORDS_AT);
    f[IR_WORDS_AT..IR_WORDS_AT + n].copy_from_slice(&packed[..n]);
    f[..8].copy_from_slice(&SYNC.to_be_bytes());
    f[8] = ((line >> 8) & 0x1F) as u8;
    f[9] = line as u8;
    f[10] = (segment & 0x3F) as u8;
    f
}

/// The line counter of a visible frame (15 bits) and whether it is below 17 200. `None` for a short frame.
pub fn vis_line(f: &[u8]) -> Option<(u32, bool)> {
    (f.len() >= VIS_BYTES).then(|| {
        let line = (u32::from(f[8]) << 8 | u32::from(f[9])) & 0x7FFF;
        (line, line < VIS_LINES)
    })
}

/// Visible pixel `i` (0 to 6003) of a frame: word 2i of the words from byte 190.
pub fn vis_pixel(f: &[u8], i: usize) -> u16 {
    word10(f, VIS_WORDS_AT, 2 * i)
}

/// The visible frame of `line` with `pixels` (6004 10-bit values; the second sample of each pixel repeats it).
pub fn build_vis(line: u32, pixels: &[u16]) -> Vec<u8> {
    let mut words = vec![0u16; 2 * VIS_PIXELS];
    for (i, &p) in pixels.iter().take(VIS_PIXELS).enumerate() {
        words[2 * i] = p & 0x3FF;
        words[2 * i + 1] = p & 0x3FF;
    }
    let mut f = vec![0u8; VIS_BYTES];
    let packed = pack10(&words);
    let n = packed.len().min(VIS_BYTES - VIS_WORDS_AT);
    f[VIS_WORDS_AT..VIS_WORDS_AT + n].copy_from_slice(&packed[..n]);
    f[..8].copy_from_slice(&SYNC.to_be_bytes());
    f[8] = ((line >> 8) & 0x7F) as u8;
    f[9] = line as u8;
    f
}

/// The 10-bit value of the test picture at channel `c`, column `x` of the full line and line `y`.
pub fn test_value(c: usize, x: usize, y: u32) -> u16 {
    let (xf, ym) = (x as f64, f64::from(y % 64));
    let v = match c % 3 {
        0 => 150.0 + 500.0 * (xf / 512.0).fract() + 100.0 * (std::f64::consts::TAU * ym / 64.0).sin(),
        1 => 200.0 + 500.0 * ym / 64.0 + 100.0 * (25.13 * xf / 512.0).cos(),
        _ => 512.0 + 350.0 * ((x % 256) as f64 / 40.0 + ym / 10.0).cos(),
    };
    v.round().clamp(0.0, 1023.0) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_infrared_frame_round_trips() {
        let px: Vec<u16> = (0..IR_CHANNELS * IR_SEGMENT)
            .map(|k| test_value(k / IR_SEGMENT, 2 * IR_SEGMENT + k % IR_SEGMENT, 7))
            .collect();
        let f = build_ir(7, 4, &px);
        assert_eq!(f.len(), IR_BYTES);
        let d = decode_ir(&f).unwrap();
        assert_eq!((d.line, d.segment, d.valid), (7, 4, true));
        assert_eq!(d.pixels, px);
    }

    #[test]
    fn a_visible_frame_round_trips() {
        let px: Vec<u16> = (0..VIS_PIXELS).map(|x| test_value(0, x, 3)).collect();
        let f = build_vis(12, &px);
        assert_eq!(vis_line(&f), Some((12, true)));
        assert!((0..VIS_PIXELS).all(|i| vis_pixel(&f, i) == px[i]));
    }
}
